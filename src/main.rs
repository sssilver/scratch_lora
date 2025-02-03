use anyhow::Result;
// use ble::Ble;
// use gps::Gps;
use lora::Lora;

mod ble;
mod gps;
mod lora;
mod nmea;
mod telemetry;
mod thread;

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("hello, small black box!");

    // let _ble_tx = Ble::spawn()?;
    // let _gps_tx = Gps::spawn()?;
    let _lora_tx = Lora::spawn()?;

    // Keep the main task running
    // let mut counter = 0;
    loop {
        // let telemetry = format!("Counter: {counter}").into_bytes();

        // ble_tx.send(ble::Message::Notify { data: telemetry })?;
        // counter += 1;

        esp_idf_svc::hal::delay::FreeRtos::delay_ms(1000);
    }
}
