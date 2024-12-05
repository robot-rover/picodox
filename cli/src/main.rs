use std::{cmp, fmt::Debug, io::{BufRead, BufReader, ErrorKind, Read, Write}, thread, time::{Duration, Instant}};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};

use crc::{Crc, CRC_8_BLUETOOTH};
use picodox_proto::{Command, Response, DATA_COUNT};
use serde::{de::DeserializeOwned, Serialize};
use serialport::SerialPort;

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
    #[command(name = "flash")]
    #[command(about = "Change the keyboard mcu into DFU flash mode")]
    FlashFw,
    #[command(about = "Reset the keyboard mcu")]
    Reset,
    #[command(about = "List all serial ports")]
    ListSerial,
    #[command(about = "Send data to the mcu over serial and read its response")]
    Echo {
        #[arg(help = "The content to send")]
        msg: String,
    },
}

fn main() {
    let args = Cli::parse();

    let res = match args.command {
        SubCommand::Reset => reset(&args.device),
        SubCommand::FlashFw => flash_fw(&args.device),
        SubCommand::ListSerial => list_serial(),
        SubCommand::Echo { msg } => send_echo(&args.device, &msg),
    };

    if let Err(err) = res {
        println!("Error: {:#}", err);
        std::process::exit(1);
    }
}

fn reset(dev: &str) -> Result<()> {
    let mut port = open_port(dev)?;
    send_command(&mut port.get_mut(), &Command::Reset)?;
    let resp: Response = recv_response(&mut port)?;
    println!("Reset Response: {resp:?}");

    Ok(())
}


fn is_picoboot_connected() -> bool {
    const RASPI_VID: u16 = 0x2e8a;
    const PICOBOOT_PID: u16 = 0x0003;

    usb_enumeration::enumerate(Some(RASPI_VID), Some(PICOBOOT_PID)).len() > 0
}

fn flash_fw(dev: &str) -> Result<()> {
    let mut port = open_port(dev)?;
    send_command(&mut port.get_mut(), &Command::UsbDfu)?;
    let resp: Response = recv_response(&mut port)?;
    println!("Reset Response: {resp:?}");

    let now = Instant::now();
    while (Instant::now() - now) < Duration::from_secs(5) {
        if is_picoboot_connected() {
            return Ok(());
        } else {
            thread::sleep(Duration::from_millis(100));
        }
    }
    return Err(anyhow!("Timeout waiting for PICOBOOT device"));
}

fn open_port(device: &str) -> Result<BufReader<Box<dyn SerialPort>>> {
    let port = serialport::new(device, 115_200)
        .timeout(SERIAL_TIMEOUT)
        .open().with_context(|| format!("Failed to open serial port '{device}'"))?;

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
    let mut cobs_decoded =  cobs::decode_vec(&read_buf)
        .ok().ok_or_else(|| anyhow!("Invalid packet encountered (illegal cobs) {:0x?}", read_buf))?;
        //bail!("")

    let actual_crc = if let Some(crc) = cobs_decoded.pop() {
        crc
    } else {
        bail!("Invalid packet encountered (missing CRC) {:0x?}", read_buf)
    };

    // Check the CRC
    let expect_crc = CRC.checksum(&cobs_decoded);
    if expect_crc != actual_crc {
        bail!("Invalid packet CRC (actual: {actual_crc:x}, expected: {expect_crc:x}) {:0x?}", read_buf);
    }

    // Finally, decode the response
    postcard::from_bytes(&cobs_decoded)
        .with_context(|| format!("Failed to deserialize response {:0x?}", read_buf))
}

fn list_serial() -> Result<()> {
    let ports = serialport::available_ports()
        .context("Unable to enumerate available serial ports")?;
    if ports.len() > 0 {
        for port in ports {
            println!("{}", port.port_name);
        }
    } else {
        println!("No serial ports detected!");
    }

    Ok(())
}

fn  send_echo(dev: &str, content: &str) -> Result<()> {
    let mut port = open_port(dev)?;

    println!("Sending '{}'", content);
    let command = Command::EchoMsg { count: content.len().try_into().context("Message is too long")? };
    send_command(&mut port.get_mut(), &command)
        .context("Sending EchoMsg command")?;

    for (idx, chunk) in content.as_bytes().chunks(DATA_COUNT).enumerate() {
        let mut data = [0u8; DATA_COUNT];
        data[..chunk.len()].copy_from_slice(chunk);
        send_command(&mut port.get_mut(), &Command::Data(data))
            .with_context(|| format!("Sending data command {}", idx))?;
    }

    let resp: Response = recv_response(&mut port)
        .context("Receiving EchoMsg response")?;

    let resp_count = match resp {
        Response::EchoMsg { count } => count as usize,
        Response::PacketErr(err) => bail!("Received Packet error waiting for EchoMsg: {:?}", err),
        other => bail!("Unexpected response: {:?}, expecting EchoMsg", other),
    };

    let mut resp_content = Vec::new();
    for i in (0..resp_count).step_by(DATA_COUNT) {
        let resp: Response = recv_response(&mut port)?;
        let resp_data = match resp {
            Response::Data(data) => data,
        Response::PacketErr(err) => bail!("Received Packet error waiting for Data: {:?}", err),
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

    use picodox_proto::{errors::{ProtoError, Ucid}, proto_impl::{self, wire_encode}, WireSize};

    use super::*;

    fn command_cases() -> Vec<Command> {
        vec![
            Command::FlashFw,
            Command::Data([0, 0, 3, 4, 5, 6, 0, 0]),
            Command::EchoMsg { count: 7 },
        ]
    }

    fn response_cases() -> Vec<Response> {
        vec![
            Response::LogMsg { count: 27 },
            Response::EchoMsg { count: 128 },
            Response::Data([1, 2, 3, 4, 5, 6, 0, 0]),
            Response::PacketErr(ProtoError::invariant(Ucid(1), 2)),
        ]
    }

    fn ser<S: Serialize + WireSize + fmt::Debug>(cli: bool, command: S) -> Vec<u8> {
        if cli {
            let mut buffer = Vec::new();
            send_command(&mut buffer, &command)
                .context("Send")
                .unwrap();
            buffer
        } else {
            proto_impl::wire_encode::<_, { Command::WIRE_MAX_SIZE }>(Ucid(0), command)
                .unwrap().into_iter().collect::<Vec<u8>>()
        }
    }

    fn des<D: DeserializeOwned + WireSize>(cli: bool, mut buffer: Vec<u8>) -> D {
        if cli {
            recv_response(&mut BufReader::new(&buffer[..]))
                .context("Recv")
                .unwrap()
        } else {
            proto_impl::wire_decode(Ucid(0), &mut buffer)
                .unwrap()
        }
    }

    fn command_round_trip(cli_ser: bool, cli_des: bool) {
        println!("=== cli_ser: {cli_ser}, cli_des: {cli_des} ===");
        for case in command_cases() {
            println!("Case: {:?}", case);
            let buffer: Vec<u8> = ser(cli_ser, &case);
            assert!(buffer.len() > 0);
            println!("Buffer: {:02x?}", buffer);
            let round_trip = des(cli_des, buffer);

            assert_eq!(case, round_trip);
        };
    }

    #[test]
    fn command_cli_cli() {
        command_round_trip(true, true);
    }

    #[test]
    fn command_cli_impl() {
        command_round_trip(true, false);
    }

    #[test]
    fn command_impl_impl() {
        command_round_trip(false, false);
    }

    #[test]
    fn command_impl_cli() {
        command_round_trip(false, true);
    }

    fn response_round_trip(cli_ser: bool, cli_des: bool) {
        println!("=== cli_ser: {cli_ser}, cli_des: {cli_des} ===");
        for case in response_cases() {
            println!("Case: {:?}", case);
            let buffer: Vec<u8> = ser(cli_ser, &case);
            assert!(buffer.len() > 0);
            println!("Buffer: {:02x?}", buffer);
            let round_trip = des(cli_des, buffer);

            assert_eq!(case, round_trip);
        };
    }

    #[test]
    fn response_cli_cli() {
        response_round_trip(true, true);
    }

    #[test]
    fn response_cli_impl() {
        response_round_trip(true, false);
    }

    #[test]
    fn response_impl_impl() {
        response_round_trip(false, false);
    }

    #[test]
    fn response_impl_cli() {
        response_round_trip(false, true);
    }

}
