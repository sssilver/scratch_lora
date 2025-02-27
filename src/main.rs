#![no_std]
#![no_main]

use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::gpio::Output;
use esp_hal::gpio::Pin;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use esp_println as _;
use gps::GpsConfig;
use gps::GPS_BAUD_RATE;
use {esp_alloc as _, esp_backtrace as _};

mod ble;
mod display;
mod gps;
mod log;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });
    esp_alloc::heap_allocator!(72 * 1024);
    let timer_group = TimerGroup::new(peripherals.TIMG0);

    let init = esp_wifi::init(
        timer_group.timer0,
        esp_hal::rng::Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();

    esp_hal_embassy::init(timer_group.timer1);

    //
    // Initialize i2c
    //

    // First, get the pins
    let sda = peripherals.GPIO17;
    let scl = peripherals.GPIO18;

    // Create config
    let config = esp_hal::i2c::master::Config::default();

    // Then create I2C with pins and config
    let i2c = esp_hal::i2c::master::I2c::new(peripherals.I2C0, config).unwrap();
    let mut i2c = i2c.with_scl(scl).with_sda(sda).into_async();

    esp_println::println!("Starting I2C bus stabilization...");

    for addr in 0x3A..=0x3E {
        let _ = i2c.write(addr, &[0]);
    }

    let mut delay = esp_hal::delay::Delay::new();

    let display = display::DisplayDevice::new(
        i2c,
        Output::new(peripherals.GPIO21, esp_hal::gpio::Level::High),
        &mut delay,
    )
    .unwrap();

    spawner.spawn(display::controller::start(display)).unwrap();
    spawner.spawn(ble::start(peripherals.BT, init)).unwrap();

    // GPS
    let config = GpsConfig {
        rx_pin: peripherals.GPIO46.degrade(),
        baud_rate: GPS_BAUD_RATE,
    };

    let gps = gps::Gps::new(peripherals.UART1, config).unwrap();
    spawner.spawn(gps::start(gps)).unwrap();
}
