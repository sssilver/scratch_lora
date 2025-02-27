use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use embassy_time::{with_timeout, Duration, Instant};
use error::GpsError;
use esp_hal::{
    gpio::AnyPin,
    peripherals::UART1,
    uart::{Config, UartRx},
    Async,
};
use nmea::parse_sentence;
use positioning::Positioning;

mod error;
mod nmea;
pub mod positioning;

pub const GPS_BAUD_RATE: u32 = 9600;
const SENTENCE_BUFFER_SIZE: usize = 256;

pub struct GpsConfig {
    pub baud_rate: u32,
    pub rx_pin: AnyPin,
}

const WATCH_BUFFER_SIZE: usize = 4;

// Static channel for latest positioning data
pub static GPS_STATE: Watch<CriticalSectionRawMutex, Option<Positioning>, WATCH_BUFFER_SIZE> =
    Watch::new();

pub type GpsStateRx = embassy_sync::watch::Receiver<
    'static,
    CriticalSectionRawMutex,
    Option<Positioning>,
    WATCH_BUFFER_SIZE,
>;

type GpsStateTx = embassy_sync::watch::Sender<
    'static,
    CriticalSectionRawMutex,
    Option<Positioning>,
    WATCH_BUFFER_SIZE,
>;

pub struct Gps {
    uart: UartRx<'static, Async>,
    sender: GpsStateTx,
    last_position_time: Option<Instant>,
}

impl Gps {
    pub fn new(uart1: UART1, config: GpsConfig) -> Result<Self, GpsError> {
        // Important: Increase FIFO threshold to avoid overflows
        let uart_config = Config::default().with_baudrate(config.baud_rate);
        // .with_rx_fifo_full_threshold(112); // Set to a higher threshold

        let uart = UartRx::new(uart1, uart_config)
            .map_err(|e| {
                defmt::error!("UART ERROR {:?}", e);
                GpsError::UartError
            })?
            .with_rx(config.rx_pin)
            .into_async();

        Ok(Self {
            uart,
            sender: GPS_STATE.sender(),
            last_position_time: None,
        })
    }

    // Function to drain any pending UART data
    async fn drain_uart(&mut self) {
        defmt::debug!("Draining UART buffer");
        let mut drain_buffer = [0u8; SENTENCE_BUFFER_SIZE];
        for _ in 0..5 {
            match with_timeout(
                Duration::from_millis(10),
                self.uart.read_async(&mut drain_buffer),
            )
            .await
            {
                Ok(Ok(_)) => {}
                _ => break,
            }
        }
    }

    async fn read_gps(&mut self) -> Result<(), GpsError> {
        let mut sentence_buffer = [0u8; SENTENCE_BUFFER_SIZE];
        let mut sentence_len = 0;
        let mut read_buffer = [0u8; 16]; // Smaller buffer size for more frequent processing
        let mut looking_for_dollar = true;
        let start_time = Instant::now();

        // Drain the buffer first
        self.drain_uart().await;

        while start_time.elapsed() < Duration::from_secs(3) {
            let read_result = with_timeout(
                Duration::from_millis(500),
                self.uart.read_async(&mut read_buffer),
            )
            .await;

            match read_result {
                Ok(Ok(bytes_read)) => {
                    if bytes_read == 0 {
                        embassy_futures::yield_now().await;
                        continue;
                    }

                    for &byte in &read_buffer[..bytes_read] {
                        if looking_for_dollar {
                            if byte == b'$' {
                                // Start a new sentence
                                sentence_len = 0;
                                sentence_buffer[sentence_len] = byte;
                                sentence_len += 1;
                                looking_for_dollar = false;
                            }
                            // Skip all bytes until we find a $
                            continue;
                        }

                        // Check for end of sentence
                        if byte == b'\r' || byte == b'\n' {
                            if sentence_len > 0 {
                                // Try to process the sentence
                                if let Ok(sentence) =
                                    core::str::from_utf8(&sentence_buffer[..sentence_len])
                                {
                                    // Clean up the sentence
                                    let sentence = sentence.trim();

                                    // Only attempt to parse if it's a position-related sentence
                                    if sentence.starts_with("$GPRMC")
                                        || sentence.starts_with("$GNRMC")
                                        || sentence.starts_with("$GPGGA")
                                        || sentence.starts_with("$GNGGA")
                                    {
                                        defmt::debug!("Processing position sentence: {}", sentence);

                                        match parse_sentence(sentence) {
                                            Ok(nmea) => match Positioning::try_from(nmea) {
                                                Ok(position) => {
                                                    defmt::info!(
                                                        "Got valid position: lat={}, lon={}",
                                                        position.latitude,
                                                        position.longitude
                                                    );
                                                    self.sender.send(Some(position));
                                                    self.last_position_time = Some(Instant::now());
                                                    return Ok(());
                                                }
                                                Err(e) => {
                                                    defmt::debug!(
                                                        "Invalid position: {:?}",
                                                        defmt::Debug2Format(&e)
                                                    );
                                                }
                                            },
                                            Err(e) => {
                                                defmt::debug!(
                                                    "Parse error: {:?}",
                                                    defmt::Debug2Format(&e)
                                                );
                                            }
                                        }
                                    } else {
                                        // Skip sentences we don't care about
                                        defmt::debug!(
                                            "Skipping non-position sentence: {}",
                                            &sentence
                                        );
                                    }
                                }
                            }
                            // Look for next sentence
                            looking_for_dollar = true;
                            continue;
                        }

                        // Handle new $ (start of new sentence) in the middle of processing
                        if byte == b'$' {
                            sentence_len = 0;
                            sentence_buffer[sentence_len] = byte;
                            sentence_len += 1;
                            looking_for_dollar = false;
                            continue;
                        }

                        // Add character to current sentence if it's valid
                        if byte.is_ascii_graphic() || byte == b' ' {
                            if sentence_len < SENTENCE_BUFFER_SIZE - 1 {
                                sentence_buffer[sentence_len] = byte;
                                sentence_len += 1;
                            } else {
                                // Sentence too long, likely corrupted
                                looking_for_dollar = true;
                                defmt::debug!("Sentence too long, discarding");
                            }
                        }
                    }
                }
                Ok(Err(e)) => {
                    defmt::error!("UART error: {:?}", e);
                    self.drain_uart().await;
                    return Err(GpsError::UartError);
                }
                Err(_) => {
                    // Read timeout - continue
                    embassy_futures::yield_now().await;
                }
            }
        }

        // Only invalidate position if we haven't had a fix for a while
        if self.last_position_time.is_none()
            || self.last_position_time.unwrap().elapsed() > Duration::from_secs(30)
        {
            self.sender.send(None);
            defmt::warn!("No GPS fix for 30 seconds");
        }

        Err(GpsError::Timeout)
    }
}

// Special error handling for common UART errors
fn handle_uart_error(error: &GpsError) {
    match error {
        GpsError::UartError => {
            // Sleep a bit longer on UART errors to let hardware recover
            defmt::warn!("UART error detected, allowing hardware recovery time");
        }
        _ => {}
    }
}

#[embassy_executor::task]
pub async fn start(mut gps: Gps) {
    defmt::info!("Starting GPS task");

    // Drain the UART buffer on startup
    gps.drain_uart().await;

    // Add a startup delay to let GPS module initialize
    embassy_time::Timer::after(Duration::from_millis(500)).await;

    let mut consecutive_errors = 0;

    loop {
        let result = gps.read_gps().await;

        match result {
            Ok(_) => {
                // Success - reset error counter
                consecutive_errors = 0;
                // Brief pause between successful reads
                embassy_time::Timer::after(Duration::from_millis(100)).await;
            }
            Err(e) => {
                // Handle errors with exponential backoff
                consecutive_errors += 1;

                // Special handling for UART errors
                handle_uart_error(&e);

                // Exponential backoff up to 1 second
                let backoff = consecutive_errors.min(10) * 100;
                defmt::debug!("GPS error, backing off for {}ms", backoff);
                embassy_time::Timer::after(Duration::from_millis(backoff)).await;
            }
        }

        embassy_futures::yield_now().await;
    }
}
