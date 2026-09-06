use crate::beslink::{
    burn_image_to_flash, detect_default_flash_address, helper_sync_and_load_programmer,
    parse_programmer_blob, resolve_programmer_binary, send_device_reboot, BESLinkError,
    BES_PROGRAMMING_BAUDRATE,
};
use crate::serial_port_opener::open_serial_port_with_wait;
use serialport::{ClearBuffer, SerialPort};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info};

pub fn cmd_write_image(
    input_file: &PathBuf,
    port_name: &str,
    programmer_path: Option<&PathBuf>,
    start_address: Option<u32>,
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

    println!(
        "Writing binary data to {port_name} @ {BES_PROGRAMMING_BAUDRATE} (Flash: 0x{:08X})",
        flash_base
    );
    let mut port = open_serial_port_with_wait(port_name, BES_PROGRAMMING_BAUDRATE, wait_for_port);
    port.set_timeout(Duration::from_millis(5000))
        .expect("Cant set port timeout");

    let _ = port.clear(ClearBuffer::All);
    info!("Starting loader and checking communications");
    match helper_sync_and_load_programmer(&mut port, &programmer_blob) {
        Ok(_) => {
            info!("Done...");
        }
        Err(e) => {
            error!("Failed {}", e);
            return;
        }
    }

    info!("Now doing firmware load");
    match do_burn_image_to_flash(input_file, &mut port, flash_base as usize) {
        Ok(_) => {
            info!("Done...");
        }
        Err(e) => {
            error!("Failed {}", e);
        }
    }
}

fn do_burn_image_to_flash(
    input_file: &PathBuf,
    serial_port: &mut Box<dyn SerialPort>,
    flash_base: usize,
) -> Result<(), BESLinkError> {
    let file_contents = fs::read(input_file)?;
    burn_image_to_flash(serial_port, file_contents, flash_base)?;
    send_device_reboot(serial_port)?;
    Ok(())
}