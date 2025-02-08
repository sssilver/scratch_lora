#![no_std]
#![no_main]

use bt_hci::controller::ExternalController;
use core::panic::PanicInfo;
use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use esp_println as _;
use esp_wifi::ble::controller::BleConnector;
use {esp_alloc as _, esp_backtrace as _};

pub const L2CAP_MTU: usize = 247;

mod ble;
mod log;

#[esp_hal_embassy::main]
async fn main(_s: Spawner) {
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });
    esp_alloc::heap_allocator!(72 * 1024);
    let timg0 = TimerGroup::new(peripherals.TIMG0);

    let init = esp_wifi::init(
        timg0.timer0,
        esp_hal::rng::Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();

    esp_hal_embassy::init(timg0.timer1);

    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(&init, bluetooth);

    let controller: ExternalController<_, 20> = ExternalController::new(connector);

    ble::run::<_, L2CAP_MTU>(controller).await;
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
