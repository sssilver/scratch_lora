use std::sync::mpsc;

use anyhow::Result;
use esp_idf_hal::{
    cpu::Core,
    gpio,
    prelude::Peripherals,
    uart::{config::Config, UartDriver},
};
use log::{error, info};

use crate::thread;
use crate::{nmea, telemetry::Telemetry};

type Rx = mpsc::Receiver<Message>;
pub type Tx = mpsc::Sender<Message>;

pub enum Message {}

pub struct Gps {
    uart_driver: UartDriver<'static>,

    history: Vec<Telemetry>,
}

impl Gps {
    fn new() -> Result<Self> {
        log::info!("initialize");

        let peripherals = Peripherals::take()?;

        let tx = peripherals.pins.gpio46;
        let rx = peripherals.pins.gpio45;

        // Configure UART for GT-U7 GPS module per datasheet
        let config = Config::default()
            .baudrate(esp_idf_hal::units::Hertz(9600))
            .data_bits(esp_idf_hal::uart::config::DataBits::DataBits8)
            .parity_none()
            .stop_bits(esp_idf_hal::uart::config::StopBits::STOP1);

        let uart_driver = UartDriver::new(
            peripherals.uart1,
            tx,
            rx,
            Option::<gpio::Gpio0>::None,
            Option::<gpio::Gpio0>::None,
            &config,
        )?;

        log::info!("UART driver created successfully");

        Ok(Self {
            uart_driver,

            history: Vec::new(),
        })
    }

    fn run(&mut self, _rx: Rx) -> Result<()> {
        const SENTENCE_SIZE: usize = 128;
        const UART_TIMEOUT: u32 = 1000; // ms

        let mut sentence_buffer = Vec::with_capacity(SENTENCE_SIZE);
        let mut read_buffer = [0u8; SENTENCE_SIZE];

        loop {
            match self.uart_driver.read(&mut read_buffer, UART_TIMEOUT) {
                Ok(bytes_read) => {
                    for &byte in &read_buffer[..bytes_read] {
                        match byte {
                            b'\n' | b'\r' => {
                                if !sentence_buffer.is_empty() {
                                    if let Some(telemetry) = nmea::parse_sentence(&sentence_buffer)
                                        .and_then(Telemetry::try_from)
                                        .ok()
                                    {
                                        info!("telemetry: {:?}", telemetry);
                                        self.history.push(telemetry);
                                    }

                                    sentence_buffer.clear();
                                }
                            }
                            // Only add non-whitespace bytes to buffer
                            b if !b.is_ascii_whitespace() => {
                                if sentence_buffer.len() < SENTENCE_SIZE {
                                    sentence_buffer.push(byte);
                                } else {
                                    error!("Sentence buffer overflow, clearing");
                                    sentence_buffer.clear();
                                }
                            }
                            _ => {} // Ignore other whitespace
                        }
                    }
                }
                Err(err) => {
                    error!("UART read error: {:?}", err);
                    esp_idf_hal::delay::FreeRtos::delay_ms(100);
                }
            }
        }
    }

    pub fn spawn() -> Result<Tx> {
        let (tx, rx) = mpsc::channel();

        let mut gps = Gps::new()?;

        let _ = thread::spawn("gps\0", Core::Core1, move || Ok(gps.run(rx)?));

        Ok(tx)
    }
}
