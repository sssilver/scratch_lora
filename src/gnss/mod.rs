mod error;
pub mod positioning;
mod sentence;

// ESP32-specific modules
#[cfg(feature = "esp32")]
pub mod driver;
#[cfg(feature = "esp32")]
pub mod watch;
