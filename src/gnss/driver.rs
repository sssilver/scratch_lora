use super::error::GnssError;
use super::positioning::GnssPositioning;
use super::sentence::SentenceBuffer;
use super::watch::{GnssStateTx, GNSS_WATCH};
use core::str;
use esp_hal::{
    gpio::AnyPin,
    peripherals::UART1,
    uart::{self, RxConfig, RxError, UartRx},
    Async,
};
use nmea::parse_str;

pub const GNSS_BAUD_RATE: u32 = 9600;

pub struct Config {
    pub baud_rate: u32,
    pub rx_pin: AnyPin,
}

pub struct Gnss {
    uart: UartRx<'static, Async>,
    sender: GnssStateTx,

    nmea_buffer: SentenceBuffer,
}

impl Gnss {
    pub fn new<'a>(uart1: UART1, config: Config) -> Result<Self, GnssError> {
        let uart_config = uart::Config::default()
            .with_baudrate(config.baud_rate)
            .with_rx(RxConfig::default().with_fifo_full_threshold(1024));

        let uart = UartRx::new(uart1, uart_config)
            .map_err(|_| GnssError::UartError)?
            .with_rx(config.rx_pin)
            .into_async();

        Ok(Self {
            uart,
            sender: GNSS_WATCH.sender(),
            nmea_buffer: SentenceBuffer::new(),
        })
    }

    fn drain_uart_buffer(&mut self) {
        defmt::debug!("Draining UART buffer");

        loop {
            let mut temp_buf = [0u8; 128]; // Buffer size to read in chunks
            match self.uart.read_buffered(&mut temp_buf) {
                Ok(0) => break,    // Stop when no more bytes are available
                Ok(_) => continue, // Keep reading if bytes were read
                Err(err) => {
                    defmt::error!("UART read error while draining: {}", err);
                    break; // Stop draining on another error
                }
            }
        }
    }

    async fn read_positioning(&mut self) -> Result<(), GnssError> {
        let mut read_buffer = [0u8; 64]; // UART read buffer

        loop {
            match self.uart.read_async(&mut read_buffer).await {
                Ok(bytes_read) if bytes_read > 0 => {
                    for &byte in &read_buffer[..bytes_read] {
                        if let Some(sentence) = self.nmea_buffer.feed(byte) {
                            defmt::info!("nmea: {}", sentence);

                            match Self::parse(sentence) {
                                Ok(positioning) => {
                                    defmt::info!("Positioning: {}", positioning);
                                    self.sender.send(Some(positioning));
                                }
                                Err(GnssError::NoFix) => self.sender.send(None),
                                Err(e) => {
                                    defmt::warn!("NMEA parse error: {:?}", defmt::Debug2Format(&e))
                                }
                            }
                        }
                    }

                    defmt::info!("{}", self.nmea_buffer.as_string().unwrap());
                }

                Ok(_) => continue, // No bytes read; continue to next iteration

                Err(e) => self.handle_uart_error(e),
            }
        }
    }

    fn parse(sentence: &str) -> Result<GnssPositioning, GnssError> {
        return parse_str(sentence)
            .map_err(|e| {
                defmt::warn!("NMEA parse error: {:?}", defmt::Debug2Format(&e));

                GnssError::ParseError
            })
            .and_then(|parsed_data| GnssPositioning::try_from(parsed_data));
    }

    fn handle_uart_error(&mut self, e: RxError) {
        defmt::warn!("UART error: {}", e);

        if let RxError::FifoOverflowed = e {
            self.drain_uart_buffer();
            self.nmea_buffer.reset("FIFO overflowed");
        }
    }
}

#[embassy_executor::task]
pub async fn start(mut gnss: Gnss) {
    defmt::info!("Starting GNSS task");

    loop {
        let result = gnss.read_positioning().await;

        match result {
            Ok(_) => {
                // Success - position found
                defmt::warn!("SUCCESS");
            }
            Err(e) => {
                defmt::warn!("FAILURE: {}", defmt::Debug2Format(&e));
            }
        }
    }
}
