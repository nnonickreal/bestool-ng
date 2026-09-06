pub mod read_image;
pub mod write_image;
pub mod write_image_then_monitor;

pub use read_image::cmd_read_image;
pub use write_image::cmd_write_image;
pub use write_image_then_monitor::cmd_write_image_then_monitor;

use crate::serial_monitor::run_serial_monitor;
use crate::serial_port_opener::open_serial_port_with_wait;

pub fn cmd_list_serial_ports() {
    match serialport::available_ports() {
        Ok(ports) => {
            for p in ports {
                println!("{}", p.port_name);
            }
        }
        Err(e) => {
            println!("Failed to list serial ports: {}", e);
        }
    }
}

pub fn cmd_serial_port_monitor(serial_port: &str, baud_rate: u32, wait_for_port: bool) {
    let port = open_serial_port_with_wait(serial_port, baud_rate, wait_for_port);
    match run_serial_monitor(port) {
        Ok(_) => {}
        Err(e) => {
            println!("Failed monitoring: {}", e);
        }
    }
}