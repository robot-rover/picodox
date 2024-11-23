use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

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
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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

    match args.command {
        Command::Reset => println!("Reset!"),
        Command::FlashFw => println!("FlashFw!"),
        Command::ListSerial => list_serial().unwrap(),
        Command::Serial { msg } => send_serial(&args.device, msg.as_bytes()).unwrap(),
    };
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
    let mut port = serialport::new(dev, 115_200)
        .timeout(Duration::from_millis(100))
        .open().with_context(|| format!("Failed to open serial port '{dev}'"))?;

    println!("Sending '{}'", String::from_utf8_lossy(content));
    port.write_all(content)
        .and_then(|_| port.write(b"\r"))
        .context("Failed to write to serial port")?;

    let mut read_buf = vec![0u8; 64];
    let bytes_recv = port.read(&mut read_buf).context("Failed to read from serial port")?;
    read_buf.truncate(bytes_recv);
    let read_content = String::from_utf8_lossy(&read_buf);
    println!("Received response: '{read_content}'");

    Ok(())
}
