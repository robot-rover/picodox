use std::{
    cmp,
    fmt::Debug,
    fs,
    io::{BufRead, BufReader, Write},
    thread,
    time::{Duration, Instant},
};

mod uf2;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};

use crc::{Crc, CRC_8_BLUETOOTH};
use picodox_proto::{AckType, Command, Response, DATA_COUNT};
use serde::{de::DeserializeOwned, Serialize};
use serialport::SerialPort;
use uf2::{Uf2Block, Uf2Flags};

const SERIAL_TIMEOUT: Duration = Duration::from_millis(100);
const CRC: Crc<u8> = Crc::<u8>::new(&CRC_8_BLUETOOTH);

#[derive(Debug, Parser)]
#[command(name = "picodox-cli")]
#[command(about = "A cli for interacting with the picodox keyboard")]
struct Cli {
    #[arg(help = "The serial port connected to the keyboard")]
    #[arg(short, long)]
    // TODO: Read this from a .env file instead?
    #[arg(default_value_t = String::from("/dev/ttyACM0"))]
    device: String,
    #[command(subcommand)]
    command: SubCommand,
}

#[derive(Debug, Subcommand)]
enum SubCommand {
    #[command(about = "Flash new firmware to the keyboard using the serial interface")]
    Flash {
        #[arg(help = "The elf file to flash")]
        path: String,
    },
    #[command(about = "Change the keyboard mcu into DFU flash mode")]
    Dfu,
    #[command(about = "Reset the keyboard mcu")]
    Reset,
    #[command(about = "List all serial ports")]
    ListSerial,
    #[command(about = "Send data to the mcu over serial and read its response")]
    Echo {
        #[arg(help = "The content to send")]
        msg: String,
    },
    #[command(about = "Analyze a UF2 file, showing its sections")]
    Uf2 {
        #[arg(help = "The file to analyze")]
        path: String,
    },
}

fn main() {
    let args = Cli::parse();

    let res = match args.command {
        SubCommand::Reset => reset(&args.device),
        SubCommand::Dfu => usb_dfu(&args.device),
        SubCommand::ListSerial => list_serial(),
        SubCommand::Echo { msg } => send_echo(&args.device, &msg),
        SubCommand::Flash { path } => flash_fw(&args.device, &path),
        SubCommand::Uf2 { path } => analyze_uf2(&path),
    };

    if let Err(err) = res {
        println!("Error: {:#}", err);
        std::process::exit(1);
    }
}

fn analyze_uf2(path: &str) -> Result<()> {
    let file_contents =
        fs::read(path).with_context(|| format!("Unable to open file '{}'", path))?;

    let blocks = Uf2Block::parse(&file_contents)?;

    let mut bounds = match blocks.first() {
        Some(block) => block.get_bounds(),
        None => bail!("No blocks in file!"),
    };

    for block in blocks[1..]
        .iter()
        .filter(|b| !b.get_flags().contains(Uf2Flags::NotMainFlash))
    {
        let new_bounds = block.get_bounds();
        bounds = if bounds.1 == new_bounds.0 {
            (bounds.0, new_bounds.1)
        } else {
            println!("{:x} ({} bytes)", bounds.0, bounds.1 - bounds.0);
            new_bounds
        };
    }

    println!("0x{:x} ({} bytes)", bounds.0, bounds.1 - bounds.0);

    Ok(())
}

fn flash_fw(dev: &str, path: &str) -> Result<()> {
    let mut port = open_port(dev)?;
    let fw_bytes = fs::read(path)?;
    let fw_len: u32 = fw_bytes.len().try_into().context("Firmware is too large")?;
    send_command(&mut port.get_mut(), &Command::FlashFw { count: fw_len })?;
    for chunk in fw_bytes.chunks(DATA_COUNT as usize) {
        let mut data = [0u8; DATA_COUNT];
        data[..chunk.len()].copy_from_slice(chunk);
        send_command(&mut port.get_mut(), &Command::Data(data))?;
    }

    Ok(())
}

fn reset(dev: &str) -> Result<()> {
    let mut port = open_port(dev)?;
    send_command(&mut port.get_mut(), &Command::Reset)?;

    Ok(())
}

fn is_picoboot_connected() -> bool {
    const RASPI_VID: u16 = 0x2e8a;
    const PICOBOOT_PID: u16 = 0x0003;

    usb_enumeration::enumerate(Some(RASPI_VID), Some(PICOBOOT_PID)).len() > 0
}

fn usb_dfu(dev: &str) -> Result<()> {
    let mut port = open_port(dev)?;
    send_command(&mut port.get_mut(), &Command::UsbDfu)?;

    let now = Instant::now();
    while (Instant::now() - now) < Duration::from_secs(5) {
        if is_picoboot_connected() {
            return Ok(());
        } else {
            thread::sleep(Duration::from_millis(100));
        }
    }
    bail!("Timeout waiting for PICOBOOT device");
}

fn open_port(device: &str) -> Result<BufReader<Box<dyn SerialPort>>> {
    let port = serialport::new(device, 115_200)
        .timeout(SERIAL_TIMEOUT)
        .open()
        .with_context(|| format!("Failed to open serial port '{device}'"))?;

    Ok(BufReader::new(port))
}

fn send_command<W: Write, S: Serialize + Debug>(port: &mut W, command: &S) -> Result<()> {
    // Serialize the command useing postcard
    let mut bytes = postcard::to_stdvec(command)
        .with_context(|| format!("Failed to serialize command: {:?}", command))?;
    // Add the CRC byte
    let crc_of_bytes = CRC.checksum(&bytes);
    bytes.push(crc_of_bytes);
    // COBS encode the command + crc
    let mut cobs = cobs::encode_vec(&bytes);
    // Add the and end of frame sentinel
    cobs.push(0u8);

    port.write(&cobs)
        .context("Unable to write command to serial port")?;

    Ok(())
}

fn recv_response<R: BufRead, D: DeserializeOwned>(port: &mut R) -> Result<D> {
    let mut read_buf = Vec::new();
    // Read until we get the end sentinel (/0 byte)
    port.read_until(0u8, &mut read_buf)
        .context("Error while reading the response body")?;

    // Extract the end sentinel
    let end_sentinel = read_buf.pop().expect("invariant violated");
    assert_eq!(end_sentinel, 0u8);

    // Decode COBS
    let mut cobs_decoded = cobs::decode_vec(&read_buf)
        .ok()
        .ok_or_else(|| anyhow!("Invalid packet encountered (illegal cobs) {:0x?}", read_buf))?;

    let actual_crc = if let Some(crc) = cobs_decoded.pop() {
        crc
    } else {
        bail!("Invalid packet encountered (missing CRC) {:0x?}", read_buf)
    };

    // Check the CRC
    let expect_crc = CRC.checksum(&cobs_decoded);
    if expect_crc != actual_crc {
        bail!(
            "Invalid packet CRC (actual: {actual_crc:x}, expected: {expect_crc:x}) {:0x?}",
            read_buf
        );
    }

    // Finally, decode the response
    postcard::from_bytes(&cobs_decoded)
        .with_context(|| format!("Failed to deserialize response {:0x?}", read_buf))
}

fn list_serial() -> Result<()> {
    let ports =
        serialport::available_ports().context("Unable to enumerate available serial ports")?;
    if ports.len() > 0 {
        for port in ports {
            println!("{}", port.port_name);
        }
    } else {
        println!("No serial ports detected!");
    }

    Ok(())
}

fn send_echo(dev: &str, content: &str) -> Result<()> {
    let mut port = open_port(dev)?;

    println!("Sending '{}'", content);
    let command = Command::EchoMsg {
        count: content.len().try_into().context("Message is too long")?,
    };
    send_command(&mut port.get_mut(), &command).context("Sending EchoMsg command")?;

    for (idx, chunk) in content.as_bytes().chunks(DATA_COUNT).enumerate() {
        let mut data = [0u8; DATA_COUNT];
        data[..chunk.len()].copy_from_slice(chunk);
        send_command(&mut port.get_mut(), &Command::Data(data))
            .with_context(|| format!("Sending data command {}", idx))?;
    }

    let resp: Response = recv_response(&mut port).context("Receiving EchoMsg response")?;

    let resp_count = match resp {
        Response::EchoMsg { count } => count as usize,
        Response::Nack(err) => bail!("Received nack waiting for EchoMsg: {:?}", err),
        other => bail!("Unexpected response: {:?}, expecting EchoMsg", other),
    };

    let mut resp_content = Vec::new();
    for i in (0..resp_count).step_by(DATA_COUNT) {
        let resp: Response = recv_response(&mut port)?;
        let resp_data = match resp {
            Response::Data(data) => data,
            Response::Nack(err) => bail!("Received nack waiting for Data: {:?}", err),
            other => bail!("Unexpected response: {:?}, expecting Data", other),
        };
        let copy_count = cmp::min(DATA_COUNT as usize, resp_count - i);
        resp_content.extend_from_slice(&resp_data[..copy_count]);
    }

    println!("Received '{}'", String::from_utf8_lossy(&resp_content));

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use picodox_proto::{
        errors::ProtoError,
        proto_impl::{self},
        KeyResponse, NackType, WireSize,
    };

    use super::*;

    const COMMAND_CASES: &[Command] = &[
        Command::FlashFw { count: 10 },
        Command::Data([0, 0, 3, 4, 5, 6, 0, 0]),
        Command::EchoMsg { count: 7 },
    ];

    const RESPONSE_CASES: &[Response] = &[
        Response::EchoMsg { count: 128 },
        Response::Data([1, 2, 3, 4, 5, 6, 0, 0]),
        Response::Nack(NackType::PacketErr(ProtoError::BufferSize)),
    ];

    fn key_cases() -> Vec<KeyResponse> {
        vec![
            KeyResponse::Response(Response::Ack(AckType::AckReset)),
            KeyResponse::keys([8u8, 12u8, 1u8]),
            KeyResponse::no_keys(),
        ]
    }

    fn ser<S: Serialize + WireSize + fmt::Debug, const N: usize>(
        case: usize,
        command: &S,
    ) -> Vec<u8> {
        match case {
            0 => {
                let mut buffer = Vec::new();
                send_command(&mut buffer, &command).context("Send").unwrap();
                buffer
            }
            1 => proto_impl::wire_encode::<_, N>(command).unwrap().to_vec(),
            2 => proto_impl::cs_encode::<_, N>(command).unwrap().to_vec(),
            _ => unimplemented!(),
        }
    }

    fn des<D: DeserializeOwned + WireSize>(case: usize, mut buffer: Vec<u8>) -> D {
        match case {
            0 => recv_response(&mut BufReader::new(&buffer[..]))
                .context("Recv")
                .unwrap(),
            1 => proto_impl::wire_decode(&mut buffer).unwrap(),
            2 => proto_impl::cs_decode(&mut buffer).unwrap(),
            _ => unimplemented!(),
        }
    }

    fn round_trip<T, const N: usize>(ser_idx: usize, des_idx: usize, cases: &[T])
    where
        T: Serialize + DeserializeOwned + WireSize + fmt::Debug + PartialEq,
    {
        println!("=== ser: {ser_idx}, des: {des_idx} ===");
        for case in cases {
            println!("Case: {:?}", case);
            let buffer: Vec<u8> = ser::<T, N>(ser_idx, &case);
            assert!(buffer.len() > 0);
            println!("Buffer: {:02x?}", buffer);
            let round_trip = des(des_idx, buffer);

            assert_eq!(case, &round_trip);
        }
    }

    #[test]
    fn command_wire_cross() {
        for ser_idx in 0..=1 {
            for des_idx in 0..=1 {
                round_trip::<Command, { Command::WIRE_MAX_SIZE }>(ser_idx, des_idx, COMMAND_CASES)
            }
        }
    }

    #[test]
    fn command_cs() {
        round_trip::<Command, { Command::CS_MAX_SIZE }>(2, 2, COMMAND_CASES)
    }

    #[test]
    fn response_wire_cross() {
        for ser_idx in 0..=1 {
            for des_idx in 0..=1 {
                round_trip::<Response, { Response::WIRE_MAX_SIZE }>(
                    ser_idx,
                    des_idx,
                    RESPONSE_CASES,
                )
            }
        }
    }

    #[test]
    fn response_cs() {
        round_trip::<Response, { Response::CS_MAX_SIZE }>(2, 2, RESPONSE_CASES)
    }

    #[test]
    fn key_response_cs() {
        round_trip::<KeyResponse, { KeyResponse::CS_MAX_SIZE }>(2, 2, &key_cases())
    }
}
