#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use esp_backtrace as _;
use esp_hal::gpio::Input;
use esp_hal::gpio::InputConfig;
use esp_hal::gpio::Level;
use esp_hal::gpio::Output;
use esp_hal::gpio::OutputConfig;
use esp_hal::gpio::Pin;
use esp_hal::spi::master::Config;
use esp_hal::spi::master::Spi;
use esp_hal::spi::Mode;
use esp_hal::time::Rate;
use esp_hal::Async;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use esp_println as _;
use lora_phy::sx126x::Sx126x;
use static_cell::StaticCell;

use {esp_alloc as _, esp_backtrace as _};

mod ble;
mod display;
mod gnss;
mod log;
mod lora;

static SPI_BUS: StaticCell<
    Mutex<CriticalSectionRawMutex, esp_hal::spi::master::Spi<'static, Async>>,
> = StaticCell::new();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 72 * 1024);
    let timer_group = TimerGroup::new(peripherals.TIMG0);

    let init = esp_wifi::init(
        timer_group.timer0,
        esp_hal::rng::Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();

    esp_hal_embassy::init(timer_group.timer1);

    //
    // Initialize SPI
    //
    let nss = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());
    let sclk = peripherals.GPIO9;
    let mosi = peripherals.GPIO10;
    let miso = peripherals.GPIO11;

    let reset = Output::new(peripherals.GPIO12, Level::Low, OutputConfig::default());
    let busy = Input::new(peripherals.GPIO13, InputConfig::default());
    let dio1 = Input::new(peripherals.GPIO14, InputConfig::default());

    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(Rate::from_khz(100))
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_miso(miso)
    .into_async();

    // Initialize the static SPI bus
    let spi_bus = SPI_BUS.init(Mutex::new(spi));

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
        Output::new(
            peripherals.GPIO21,
            esp_hal::gpio::Level::High,
            OutputConfig::default(),
        ),
        &mut delay,
    )
    .unwrap();

    spawner.spawn(display::controller::start(display)).unwrap();
    spawner.spawn(ble::start(peripherals.BT, init)).unwrap();
    spawner
        .spawn(lora::start(spi_bus, nss, reset, dio1, busy))
        .unwrap();

    // GPS
    let config = gnss::driver::Config {
        rx_pin: peripherals.GPIO46.degrade(),
        baud_rate: gnss::driver::GNSS_BAUD_RATE,
    };

    let gps = gnss::driver::Gnss::new(peripherals.UART1, config).unwrap();
    spawner.spawn(gnss::driver::start(gps)).unwrap();
}
