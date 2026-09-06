use crate::beslink::message::read_message;
use crate::beslink::{send_message, sync, BESLinkError, BesMessage, MessageTypes, BES_SYNC};
use crc::{Crc, CRC_32_ISO_HDLC};
use serialport::SerialPort;
use std::convert::TryInto;
use std::fs;
use std::io::Write;
use std::path::Path;
use tracing::{error, info, warn};

const DEFAULT_PROGRAMMER_BINARY: &[u8] = include_bytes!("../../../programmer.bin");

#[derive(Debug, Clone, Copy)]
pub struct ProgrammerBlobInfo {
    pub entry_address: u32,
    pub payload_offset: usize,
    pub payload_length: u32,
    pub start_crc32: u32,
    pub leader_seed: [u8; 4],
}

pub fn detect_default_flash_address(entry_address: u32) -> u32 {
    if entry_address < 0x2005_0000 {
        0x3C00_0000
    } else {
        0x2C00_0000
    }
}

pub fn resolve_programmer_binary(
    custom_path: Option<&impl AsRef<Path>>,
) -> Result<Vec<u8>, BESLinkError> {
    match custom_path {
        Some(path) => {
            info!("Loading custom programmer binary from {:?}", path.as_ref());
            fs::read(path).map_err(BESLinkError::from)
        }
        None => {
            info!("Using built-in default programmer binary");
            Ok(DEFAULT_PROGRAMMER_BINARY.to_vec())
        }
    }
}

pub fn parse_programmer_blob(programmer_binary: &[u8]) -> Result<ProgrammerBlobInfo, BESLinkError> {
    if programmer_binary.len() < 16 {
        return Err(BESLinkError::InvalidArgs);
    }

    let entry_address =
        u32::from_le_bytes(programmer_binary[programmer_binary.len() - 4..].try_into().unwrap());
    let entry_bytes = entry_address.to_le_bytes();

    let payload_offset = programmer_binary
        .windows(12)
        .position(|window| window[0..8] == [0x00; 8] && window[8..12] == entry_bytes)
        .map(|offset| offset + 8)
        .ok_or(BESLinkError::InvalidArgs)?;

    if payload_offset < 12 || payload_offset >= programmer_binary.len() - 4 {
        return Err(BESLinkError::InvalidArgs);
    }

    let payload = &programmer_binary[payload_offset..programmer_binary.len() - 4];
    let payload_length = (payload.len() + 0x0C) as u32;
    let start_crc_slice = &programmer_binary[payload_offset - 12..programmer_binary.len() - 4];
    let leader_seed = programmer_binary[payload_offset - 12..payload_offset - 8]
        .try_into()
        .unwrap();

    let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    let mut digest = crc.digest();
    digest.update(start_crc_slice);

    Ok(ProgrammerBlobInfo {
        entry_address,
        payload_offset,
        payload_length,
        start_crc32: digest.finalize(),
        leader_seed,
    })
}

fn get_sync_ack_message(stage: u8) -> BesMessage {
    let mut msg = BesMessage {
        sync: BES_SYNC,
        type1: MessageTypes::Sync,
        payload: vec![stage, 0x01, 0x01],
        checksum: 0x00,
    };
    msg.set_checksum();
    msg
}

pub fn load_programmer_runtime_binary_blob(
    serial_port: &mut Box<dyn SerialPort>,
    programmer_binary: &[u8],
) -> Result<bool, BESLinkError> {
    let blob_info = parse_programmer_blob(programmer_binary)?;
    info!(
        "Programmer blob entry=0x{:08X} payload_offset=0x{:X} payload_length=0x{:X} leader_seed={:02X?}",
        blob_info.entry_address, blob_info.payload_offset, blob_info.payload_length, blob_info.leader_seed
    );

    let mut preload_setup_message = BesMessage {
        sync: BES_SYNC,
        type1: MessageTypes::StartProgrammer,
        payload: vec![0x00, 0x0C],
        checksum: 0x00,
    };
    preload_setup_message.payload.extend(blob_info.entry_address.to_le_bytes());
    preload_setup_message.payload.extend(blob_info.payload_length.to_le_bytes());
    preload_setup_message.payload.extend(blob_info.start_crc32.to_le_bytes());
    preload_setup_message.set_checksum();

    info!("Start Message {:X?}", preload_setup_message.to_vec());
    send_message(serial_port, preload_setup_message)?;

    let mut already_running = false;
    let response = loop {
        match read_message(serial_port) {
            Ok(resp) => {
                if resp.type1 == MessageTypes::StartProgrammer {
                    break resp;
                } else if resp.type1 == MessageTypes::ProgrammerInit {
                    info!("Detected ProgrammerInit (0x60). The programmer is already running on the device!");
                    already_running = true;
                    break resp;
                } else if resp.type1 == MessageTypes::Sync {
                    if resp.payload.len() >= 3 {
                        let stage = resp.payload[0];
                        let state = resp.payload[2];
                        if state == 0x00 {
                            let ack = get_sync_ack_message(stage);
                            send_message(serial_port, ack)?;
                            info!("Sent runtime sync ack for stage 0x{:02X} while waiting for StartProgrammer", stage);
                        }
                    }
                } else {
                    warn!("Ignored packet type {:?} waiting for StartProgrammer => {:X?}", resp.type1, resp.to_vec());
                }
            }
            Err(e) => return Err(e),
        }
    };

    if already_running {
        return Ok(true);
    }

    if response.payload.is_empty() || response.payload[0] != 0x00 {
        return Err(BESLinkError::BadResponseCode {
            failed_packet: response.to_vec(),
            got: response.payload.first().copied().unwrap_or_default(),
            wanted: 0,
        });
    }

    let mut stream_start_msg = BesMessage {
        sync: BES_SYNC,
        type1: MessageTypes::ProgrammerRunning,
        payload: vec![0xA2, 0x03, 0x00, 0x00, 0x00],
        checksum: 0x00,
    };
    stream_start_msg.set_checksum();
    send_message(serial_port, stream_start_msg)?;

    let raw_data = &programmer_binary[blob_info.payload_offset - 12..programmer_binary.len() - 4];
    match serial_port.write_all(raw_data) {
        Ok(_) => info!("Wrote {} bytes of raw binary stream", raw_data.len()),
        Err(e) => {
            error!("Failed to write the programmer binary {:?}", e);
            return Err(BESLinkError::from(e));
        }
    }

    let response = crate::beslink::sync(serial_port, MessageTypes::ProgrammerRunning)?;
    if response.payload.len() < 2 || response.payload[0] != 0xA2 || response.payload[1] != 0x01 {
        return Err(BESLinkError::BadResponseCode {
            failed_packet: response.to_vec(),
            got: response.payload.get(1).copied().unwrap_or_default(),
            wanted: 0x01,
        });
    }

    Ok(false)
}

pub fn start_programmer_runtime_binary_blob(
    serial_port: &mut Box<dyn SerialPort>,
) -> Result<BesMessage, BESLinkError> {
    let preload_setup_message = BesMessage {
        sync: BES_SYNC,
        type1: MessageTypes::ProgrammerStart,
        payload: vec![0x01, 0x00],
        checksum: 0xEB,
    };
    send_message(serial_port, preload_setup_message)?;
    info!("Sent start programmer message");

    loop {
        let resp = read_message(serial_port)?;
        match resp.type1 {
            MessageTypes::ProgrammerInit => {
                if !resp.payload.is_empty() {
                    info!("Programmer initialized with status code: 0x{:02X}", resp.payload[0]);
                }
                return Ok(resp);
            }
            MessageTypes::Sync => {
                if resp.payload.len() >= 3 {
                    let stage = resp.payload[0];
                    let state = resp.payload[2];
                    if state == 0x00 {
                        let ack = get_sync_ack_message(stage);
                        send_message(serial_port, ack)?;
                        info!("Sent runtime sync ack for stage 0x{:02X}", stage);
                    }
                }
            }
            _ => {
                warn!("Ignored packet type {:?}: {:X?}", resp.type1, resp.to_vec());
            }
        }
    }
}

pub fn sync_into_bootloader(serial_port: &mut Box<dyn SerialPort>) -> Result<(), BESLinkError> {
    info!("Spamming sync messages to catch bootloader...");
    let sync_msg = BesMessage {
        sync: BES_SYNC,
        type1: MessageTypes::Sync,
        payload: vec![0x00, 0x01, 0x01],
        checksum: 0xEF,
    };

    let original_timeout = serial_port.timeout();
    serial_port.set_timeout(std::time::Duration::from_millis(50)).unwrap();

    let mut synced = false;
    
    for attempt in 1..=100 {
        let _ = send_message(serial_port, sync_msg.clone());
        
        match read_message(serial_port) {
            Ok(resp) => {
                if resp.type1 == MessageTypes::Sync {
                    info!("Successfully synced with bootloader on attempt {}", attempt);
                    synced = true;
                    break;
                }
            }
            Err(_) => continue,
        }
    }

    serial_port.set_timeout(original_timeout).unwrap();

    if synced {
        Ok(())
    } else {
        error!("Failed to sync with bootloader after 100 attempts. Please reset the device.");
        Err(BESLinkError::InvalidArgs)
    }
}

pub fn helper_sync_and_load_programmer(
    serial_port: &mut Box<dyn SerialPort>,
    programmer_binary: &[u8],
) -> Result<(), BESLinkError> {
    sync_into_bootloader(serial_port)?;
    
    let already_running = load_programmer_runtime_binary_blob(serial_port, programmer_binary)?;
    if already_running {
        info!("Skipping programmer start command since it is already running.");
    } else {
        start_programmer_runtime_binary_blob(serial_port)?;
    }
    
    Ok(())
}