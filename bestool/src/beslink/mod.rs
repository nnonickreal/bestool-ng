pub mod errors;
pub mod helper_sync_and_load_programmer;
pub mod message;
pub mod read_flash;
pub mod reboot;
pub mod sync;
pub mod write_flash;

pub use errors::BESLinkError;
pub use helper_sync_and_load_programmer::*;
pub use message::*;
pub use read_flash::read_flash_data;
pub use reboot::send_device_reboot;
pub use sync::*;
pub use write_flash::burn_image_to_flash;

pub const BES_PROGRAMMING_BAUDRATE: u32 = 921600;
pub const BES_SYNC: u8 = 0xBE;
pub const FLASH_BUFFER_SIZE: usize = 32768;