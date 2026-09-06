mod beslink;
mod cmds;
mod serial_monitor;
mod serial_port_opener;

use crate::cmds::{
    cmd_list_serial_ports, cmd_read_image, cmd_serial_port_monitor, cmd_write_image,
    cmd_write_image_then_monitor,
};
use clap::Parser;
use std::path::PathBuf;
use tracing::Level;

/// Hexadecimal argument parser (supports "0x2C000000", "0X2C000000", "2C000000")
fn parse_hex_u32(s: &str) -> Result<u32, String> {
    let clean_str = s.trim();
    let hex_str = clean_str
        .strip_prefix("0x")
        .or_else(|| clean_str.strip_prefix("0X"))
        .unwrap_or(clean_str);
    u32::from_str_radix(hex_str, 16)
        .map_err(|e| format!("Invalid hex value '{}': {}", s, e))
}

#[derive(Parser, Debug)]
#[command(name = "bestool")]
#[command(bin_name = "bestool")]
enum BesTool {
    ListSerialPorts(ListSerialPorts),
    SerialMonitor(SerialMonitor),
    WriteImage(WriteImage),
    WriteImageThenMonitor(WriteImageThenMonitor),
    ReadImage(ReadImage),
}

#[derive(clap::Args, Debug)]
struct ListSerialPorts {}

#[derive(clap::Args, Debug)]
struct SerialMonitor {
    serial_port_path: String,
    #[arg(short, long, default_value_t = 2000000)]
    baud_rate: u32,
    #[arg(short, long, default_value_t = false)]
    wait: bool,
}

#[derive(clap::Args, Debug)]
struct WriteImage {
    firmware_path: PathBuf,
    #[arg(short, long)]
    port: String,
    /// Path to a custom programmer.bin binary
    #[arg(short = 'P', long)]
    programmer: Option<PathBuf>,
    /// Flash base address in hex (e.g., 0x2C000000 or 0x3C000000)
    #[arg(short = 'a', long, value_parser = parse_hex_u32)]
    start_address: Option<u32>,
    #[arg(short, long, default_value_t = false)]
    wait: bool,
}

#[derive(clap::Args, Debug)]
struct WriteImageThenMonitor {
    firmware_path: PathBuf,
    #[arg(short, long)]
    port: String,
    /// Path to a custom programmer.bin binary
    #[arg(short = 'P', long)]
    programmer: Option<PathBuf>,
    /// Flash base address in hex
    #[arg(short = 'a', long, value_parser = parse_hex_u32)]
    start_address: Option<u32>,
    #[arg(short, long, default_value_t = 2000000)]
    monitor_baud_rate: u32,
    #[arg(short, long, default_value_t = false)]
    wait: bool,
}

#[derive(clap::Args, Debug)]
struct ReadImage {
    firmware_path: PathBuf,
    #[arg(short, long)]
    port: String,
    /// Path to a custom programmer.bin binary
    #[arg(short = 'P', long)]
    programmer: Option<PathBuf>,
    /// Flash base address for reading in hex (e.g. 0x2C000000 / 0x3C000000)
    #[arg(short = 'a', long, value_parser = parse_hex_u32)]
    start_address: Option<u32>,
    /// Dump length to read in hex
    #[arg(short = 'l', long, value_parser = parse_hex_u32, default_value = "0x400000")]
    length: u32,
    /// Offset from the base address in hex
    #[arg(short = 'o', long, value_parser = parse_hex_u32, default_value = "0x0")]
    offset: u32,
    #[arg(short, long, default_value_t = false)]
    wait: bool,
}

fn main() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    match BesTool::parse() {
        BesTool::ListSerialPorts(_) => cmd_list_serial_ports(),
        BesTool::SerialMonitor(args) => {
            cmd_serial_port_monitor(&args.serial_port_path, args.baud_rate, args.wait);
        }
        BesTool::WriteImage(args) => cmd_write_image(
            &args.firmware_path,
            &args.port,
            args.programmer.as_ref(),
            args.start_address,
            args.wait,
        ),
        BesTool::ReadImage(args) => cmd_read_image(
            &args.firmware_path,
            &args.port,
            args.programmer.as_ref(),
            args.start_address,
            args.offset as usize,
            args.length as usize,
            args.wait,
        ),
        BesTool::WriteImageThenMonitor(args) => cmd_write_image_then_monitor(
            &args.firmware_path,
            &args.port,
            args.programmer.as_ref(),
            args.start_address,
            args.monitor_baud_rate,
            args.wait,
        ),
    }
}