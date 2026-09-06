use crate::beslink::{
    detect_default_flash_address, helper_sync_and_load_programmer, parse_programmer_blob,
    read_flash_data, resolve_programmer_binary, send_device_reboot, BESLinkError,
    BES_PROGRAMMING_BAUDRATE, BesMessage, MessageTypes, BES_SYNC,
};
use crate::serial_port_opener::open_serial_port_with_wait;
use serialport::{ClearBuffer, SerialPort};
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use tracing::{error, info};

pub fn cmd_read_image(
    input_file: &PathBuf,
    port_name: &str,
    programmer_path: Option<&PathBuf>,
    start_address: Option<u32>,
    offset: usize,
    length: usize,
    wait_for_port: bool,
) {
    let programmer_blob = match resolve_programmer_binary(programmer_path) {
        Ok(bin) => bin,
        Err(e) => {
            error!("Failed to load programmer binary: {}", e);
            return;
        }
    };

    let blob_info = match parse_programmer_blob(&programmer_blob) {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to parse programmer binary: {}", e);
            return;
        }
    };

    let flash_base = start_address.unwrap_or_else(|| {
        let addr = detect_default_flash_address(blob_info.entry_address);
        info!("Auto-detected flash base address: 0x{:08X}", addr);
        addr
    });

    let target_start = (flash_base as usize) + offset;
    println!(
        "Reading binary data from {port_name} @ {BES_PROGRAMMING_BAUDRATE} (0x{:X} bytes from 0x{:08X})",
        length, target_start
    );

    let mut port = open_serial_port_with_wait(port_name, BES_PROGRAMMING_BAUDRATE, wait_for_port);
    port.set_timeout(Duration::from_millis(5000))
        .expect("Cant set port timeout");

    match do_read_flash_data(input_file, &mut port, &programmer_blob, target_start, length) {
        Ok(_) => {
            info!("Done...");
        }
        Err(e) => {
            error!("Failed {}", e);
        }
    }
}

fn do_read_flash_data(
    output_file_path: &PathBuf,
    serial_port: &mut Box<dyn SerialPort>,
    programmer_blob: &[u8],
    start: usize,
    length: usize,
) -> Result<(), BESLinkError> {
    let mut flash_content: Vec<u8> = vec![];
    const MAX_READ_BEFORE_RESET: usize = 1024 * 1024; // 1MiB chunks

    while flash_content.len() < length {
        let chunk_length = if (length - flash_content.len()) < MAX_READ_BEFORE_RESET {
            length - flash_content.len()
        } else {
            MAX_READ_BEFORE_RESET
        };

        let pos = start + flash_content.len();
        let is_last_chunk = (flash_content.len() + chunk_length) >= length;

        info!(
            "===== Preparing to read flash from 0x{:X} ({}%) to 0x{:X} ({}%) =====",
            pos,
            flash_content.len() * 100 / length,
            pos + chunk_length,
            (flash_content.len() + chunk_length) * 100 / length,
        );

        let chunk = do_reset_sync_read(serial_port, programmer_blob, pos, chunk_length, is_last_chunk)?;
        flash_content.extend(chunk);
    }

    let mut file = File::create(output_file_path)?;
    file.write_all(flash_content.as_slice())?;
    Ok(())
}

fn do_reset_sync_read(
    serial_port: &mut Box<dyn SerialPort>,
    programmer_blob: &[u8],
    start: usize,
    length: usize,
    is_last_chunk: bool,
) -> Result<Vec<u8>, BESLinkError> {
    info!("Starting loader and checking communications");
    
    helper_sync_and_load_programmer(serial_port, programmer_blob)?;
    info!("Done...Bootloader start");
    
    info!("Now doing flash read");
    let flash_content = read_flash_data(serial_port, start, length)?;
    
    info!("Sending reboot command...");
    let _ = send_device_reboot(serial_port);
    
    if !is_last_chunk {
        info!("Blindly spamming 30 sync packets to catch the rebooting chip for the next chunk...");
        let sync_msg = BesMessage {
            sync: BES_SYNC,
            type1: MessageTypes::Sync,
            payload: vec![0x00, 0x01, 0x01],
            checksum: 0xEF,
        };
        let packet = sync_msg.to_vec();
        
        for _ in 0..30 {
            let _ = serial_port.write_all(&packet);
            let _ = serial_port.flush();
            sleep(Duration::from_millis(30));
        }
        let _ = serial_port.clear(ClearBuffer::Input);
    } else {
        info!("Final chunk read. Letting the chip reboot normally.");
        sleep(Duration::from_millis(500));
    }
    
    Ok(flash_content)
}