use std::{fmt::Debug, io::{BufRead, BufReader, ErrorKind, Read, Write}, time::Duration};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};

use crc::{Crc, CRC_8_BLUETOOTH};
use picodox_proto::{Command, Response};
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
    #[command(about = "Reset the keyboard mcu")]
    Reset,
    #[command(name = "flash")]
    #[command(about = "Change the keyboard mcu into DFU flash mode")]
    FlashFw,
    #[command(about = "List all serial ports")]
    ListSerial,
    #[command(about = "Send data to the mcu over serial and read its response")]
    Serial {
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
        SubCommand::Serial { msg } => send_serial(&args.device, msg.as_bytes()),
    };

    // TODO: Have prettier error handling
    res.unwrap();
}

fn reset(dev: &str) -> Result<()> {
    let mut port = open_port(dev)?;
    send_command(&mut port.get_mut(), &Command::Reset)?;

    Ok(())
}

fn flash_fw(dev: &str) -> Result<()> {
    let mut port = open_port(dev)?;
    send_command(&mut port.get_mut(), &Command::FlashFw)?;

    Ok(())
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
    // Add the start and end of frame sentinels
    cobs.insert(0, 0u8);
    cobs.push(0u8);

    port.write(&cobs)
        .context("Unable to write command to serial port")?;

    Ok(())
}

fn recv_response<R: BufRead, D: DeserializeOwned>(port: &mut R) -> Result<D> {
    // Skip garbage until we get the sentiel (/0 byte)
    let mut read_buf = Vec::new();
    let garbage_count = port.read_until(0u8, &mut read_buf)
        .context("Error while looking for response begin sentinel")?;
    if garbage_count > 1 {
        // TODO: Use a real logging system
        println!("Warning: Encountered {} garbage bytes in the response stream", garbage_count - 1);
    }
    read_buf.clear(); // Clear out the garbage bytes and the begin sentinel
    port.read_until(0u8, &mut read_buf)
        .context("Error while reading the response body")?;

    // Extract the end sentinel
    let end_sentinel = read_buf.pop().expect("invariant violated");
    assert_eq!(end_sentinel, 0u8);

    // Decode COBS
    let new_len = if let Ok(new_len) = cobs::decode_in_place(&mut read_buf) {
        new_len
    } else {
        bail!("Invalid packet encountered (illegal cobs)")
    };
    read_buf.truncate(new_len);

    let actual_crc = if let Some(crc) = read_buf.pop() {
        crc
    } else {
        bail!("Invalid packet encountered (missing CRC)")
    };

    // Check the CRC
    let expect_crc = CRC.checksum(&read_buf);
    if expect_crc != actual_crc {
        bail!("Invalid packet CRC (actual: {actual_crc:x}, expected: {expect_crc:x})");
    }

    // Finally, decode the response
    postcard::from_bytes(&read_buf)
        .context("Failed to deserialize response")
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

fn send_serial(dev: &str, content: &[u8]) -> Result<()> {
    let mut port = open_port(dev)?;

    println!("Sending '{}'", String::from_utf8_lossy(content));
    port.get_mut().write_all(content)
        .and_then(|_| port.get_mut().write(b"\r"))
        .context("Failed to write to serial port")?;

    let mut read_buf = Vec::new();
    match port.read_to_end(&mut read_buf) {
        Ok(_count) => unreachable!(),
        Err(err) if err.kind() == ErrorKind::TimedOut => {},
        Err(err) => return Err(anyhow!(err).context("Failed to read from serial port")),
    };
    let read_content = String::from_utf8_lossy(&read_buf);
    println!("Received response: '{read_content}'");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_round_trip() {
        const CASES: &[Command] = &[
            Command::Reset,
            Command::FlashFw,
        ];

        for case in CASES {
            let context = || format!("Test case: {:?}", case);
            let mut buffer: Vec<u8> = Vec::new();
            send_command(&mut buffer, case)
                .with_context(context)
                .unwrap();
            assert!(buffer.len() > 0);
            println!("Buffer: {:02x?}", buffer);
            let round_trip: Command = recv_response(&mut BufReader::new(&buffer[..]))
                .with_context(context)
                .unwrap();

            assert_eq!(case, &round_trip);
        };
    }

    #[test]
    fn response_round_trip() {
        const CASES: &[Response] = &[
            Response::LogMsg { bytes_count: 27 },
            Response::Data([1, 2, 3, 4, 5, 6, 0, 0]),
            Response::PacketErr,
        ];

        for case in CASES {
            let context = || format!("Test case: {:?}", case);
            let mut buffer: Vec<u8> = Vec::new();
            send_command(&mut buffer, case)
                .with_context(context)
                .unwrap();
            assert!(buffer.len() > 0);
            println!("Buffer: {:02x?}", buffer);
            let round_trip: Response = recv_response(&mut BufReader::new(&buffer[..]))
                .with_context(context)
                .unwrap();

            assert_eq!(case, &round_trip);
        };
    }

}
