use std::sync::mpsc;

use anyhow::Result;
use esp_idf_svc::hal::{
    cpu::Core,
    gpio::{InputPin, PinDriver},
    prelude::Peripherals,
    spi::{SpiConfig, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
    units::FromValueType,
};
use lora_phy::{
    iv::GenericSx126xInterfaceVariant,
    sx126x::{self, Sx1262, Sx126x, TcxoCtrlVoltage},
    LoRa,
};

use crate::thread;

type Rx = mpsc::Receiver<Message>;
pub type Tx = mpsc::Sender<Message>;

pub enum Message {}

pub struct Lora {}

impl Lora {
    fn new() -> Result<Self> {
        log::info!("initialize");

        let peripherals = Peripherals::take()?;

        // SPI pins
        let sck = peripherals.pins.gpio9;
        let mosi = peripherals.pins.gpio10;
        let miso = peripherals.pins.gpio11;
        let cs = peripherals.pins.gpio8;

        // Control pins with PinDriver
        let reset = PinDriver::output(peripherals.pins.gpio12)?;
        let busy = PinDriver::input(peripherals.pins.gpio13.downgrade_input())?;
        let dio1 = PinDriver::input(peripherals.pins.gpio14.downgrade_input())?;

        // Initialize SPI
        let spi = peripherals.spi2;
        let driver = SpiDriver::new(spi, sck, mosi, Some(miso), &SpiDriverConfig::new())?;

        let device_driver_config = SpiConfig::default().baudrate(1.MHz().into());
        let device_driver = SpiDeviceDriver::new(driver, Some(cs), &device_driver_config)?;

        // Create and configure LoRa device
        let config = sx126x::Config {
            chip: Sx1262,
            tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
            use_dcdc: true,
            rx_boost: false,
        };

        let iv = GenericSx126xInterfaceVariant::new(reset, dio1, busy, None, None).unwrap();
        let radio_kind = Sx126x::new(device_driver, iv, config);

        // NOTE: We need to call the below method in an async method, and we are not in such a one
        // Might want to consider delaying the instantiation of the LoRa radio, or turn this method into async
        let lora = LoRa::new(radio_kind, false, embassy_time::Delay); // .unwrap();

        Ok(Self {})
    }

    fn run(&mut self, _rx: Rx) -> Result<()> {
        Ok(())
    }

    pub fn spawn() -> Result<Tx> {
        let (tx, rx) = mpsc::channel();

        let mut lora = Lora::new()?;

        let _ = thread::spawn("lora\0", Core::Core1, move || Ok(lora.run(rx)?));

        Ok(tx)
    }
}
