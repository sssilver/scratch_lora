/// The packet likely should look like this
///
/// ```
/// #[repr(C, packed)]
/// struct GpsData {
///     latitude: u32,
///     longitude: u32,
///     speed: u16,
///     heading: u16,
/// }
/// ```
use core::str;

use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Input, Output};
use esp_hal::Async;
use lora_phy::iv::GenericSx126xInterfaceVariant;
use lora_phy::mod_params::{
    Bandwidth, CodingRate, ModulationParams, PacketParams, RadioError, SpreadingFactor,
};
use lora_phy::sx126x::{self, Sx1262, TcxoCtrlVoltage};
use lora_phy::{LoRa, RxMode};

use crate::Sx126x;

const RX_BUFFER_SIZE: usize = 128;
const LORA_FREQUENCY: u32 = 915_000_000; // 915 MHz (USA)
                                         // const LORA_FREQUENCY: u32 = 903_900_000;

// Configuration parameters for the LoRa interface
pub struct LoraConfig {
    pub frequency: u32,
    pub spreading_factor: SpreadingFactor,
    pub bandwidth: Bandwidth,
    pub coding_rate: CodingRate,
}

impl Default for LoraConfig {
    fn default() -> Self {
        Self {
            frequency: LORA_FREQUENCY,
            spreading_factor: SpreadingFactor::_10,
            bandwidth: Bandwidth::_250KHz,
            coding_rate: CodingRate::_4_8,
        }
    }
}

/// Error type for LoRa operations
#[derive(Debug)]
pub enum LoraError {
    /// Radio hardware error
    Radio(RadioError),
    /// Timeout during operation
    Timeout,
    /// Invalid configuration
    InvalidConfig,
    /// Buffer error (too small, overflow, etc.)
    BufferError,
    /// No data available
    NoData,
    /// Transmission error
    TransmissionError,
}

impl From<RadioError> for LoraError {
    fn from(e: RadioError) -> Self {
        LoraError::Radio(e)
    }
}

pub struct Lora<'a> {
    lora: LoRa<
        Sx126x<
            embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice<
                'a,
                CriticalSectionRawMutex,
                esp_hal::spi::master::Spi<'a, Async>,
                Output<'a>,
            >,
            GenericSx126xInterfaceVariant<Output<'a>, Input<'a>>,
            Sx1262,
        >,
        embassy_time::Delay,
    >,
    modulation_params: ModulationParams,
    packet_params: PacketParams,
    rx_buffer: [u8; RX_BUFFER_SIZE],
}

impl<'a> Lora<'a> {
    /// Create a new LoRa instance with the Embassy SPI device
    pub async fn new(
        spi_device: embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice<
            'a,
            CriticalSectionRawMutex,
            esp_hal::spi::master::Spi<'a, Async>,
            Output<'a>,
        >,
        reset: Output<'a>,
        dio1: Input<'a>,
        busy: Input<'a>,
        config: LoraConfig,
    ) -> Result<Self, LoraError> {
        // Create the interface variant
        let iv = match GenericSx126xInterfaceVariant::new(reset, dio1, busy, None, None) {
            Ok(iv) => iv,
            Err(_) => return Err(LoraError::InvalidConfig),
        };

        // Create the SX126x configuration
        let sx126x_config = sx126x::Config {
            chip: Sx1262,
            tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V7),
            use_dcdc: false,
            rx_boost: true,
        };

        // Create the radio instance
        let radio = Sx126x::new(spi_device, iv, sx126x_config);
        let mut lora = LoRa::new(radio, false, embassy_time::Delay).await?;

        let modulation_params = lora.create_modulation_params(
            config.spreading_factor,
            config.bandwidth,
            config.coding_rate,
            config.frequency,
        )?;

        let packet_params = lora.create_rx_packet_params(
            4,
            false,
            RX_BUFFER_SIZE as u8,
            true,
            false,
            &modulation_params,
        )?;

        Ok(Self {
            lora,
            modulation_params,
            packet_params,
            rx_buffer: [0; RX_BUFFER_SIZE],
        })
    }

    async fn receive(&mut self) {
        self.lora
            .prepare_for_rx(
                RxMode::Continuous,
                &self.modulation_params,
                &self.packet_params,
            )
            .await
            .unwrap();

        loop {
            match self.lora.rx(&self.packet_params, &mut self.rx_buffer).await {
                Ok((received_len, _rx_pkt_status)) => {
                    if let Ok(text) = str::from_utf8(&self.rx_buffer[..received_len as usize]) {
                        defmt::info!("Received: {}", text);
                    } else {
                        defmt::warn!(
                            "Received non-UTF8 data: {:?}",
                            &self.rx_buffer[..received_len as usize]
                        );
                    }
                }
                Err(err) => defmt::error!("rx unsuccessful = {}", err),
            }
        }
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), LoraError> {
        self.lora
            .prepare_for_tx(&self.modulation_params, &mut self.packet_params, 20, &data)
            .await?;

        match self.lora.tx().await {
            Ok(()) => {
                defmt::info!("TX DONE");

                Ok(())
            }
            Err(err) => {
                defmt::error!("Radio error = {}", err);
                Err(LoraError::TransmissionError)
            }
        }
    }

    async fn receive_for_duration(&mut self, duration: Duration) {
        defmt::info!(
            "Listening for incoming packets for {} ms",
            duration.as_millis()
        );

        // Prepare for receiving
        if let Err(e) = self
            .lora
            .prepare_for_rx(
                RxMode::Continuous,
                &self.modulation_params,
                &self.packet_params,
            )
            .await
        {
            defmt::error!("Failed to prepare for RX: {}", e);
            return;
        }

        match select(
            self.lora.rx(&self.packet_params, &mut self.rx_buffer),
            Timer::after(duration),
        )
        .await
        {
            Either::First(result) => match result {
                Ok((received_len, _rx_pkt_status)) => {
                    if let Ok(text) = core::str::from_utf8(&self.rx_buffer[..received_len as usize])
                    {
                        defmt::info!("Received: {}", text);
                    } else {
                        defmt::warn!(
                            "Received non-UTF8 data: {:?}",
                            &self.rx_buffer[..received_len as usize]
                        );
                    }
                }
                Err(err) => {
                    defmt::error!("RX error: {}", err);
                }
            },
            Either::Second(_) => {
                // Timeout occurred, duration has elapsed
                defmt::debug!("Receive time elapsed");
            }
        }
    }

    /// Main run loop - alternates between listening for 5 seconds and sending "hello"
    pub async fn run(&mut self) {
        defmt::info!("Starting LoRa operation - listen for 5s, then send 'hello'");

        loop {
            // First, listen for incoming packets for 5 seconds
            self.receive_for_duration(Duration::from_secs(5)).await;

            // Then send "hello"
            defmt::info!("5 seconds elapsed, sending 'hello'");
            if let Err(e) = self.send("hello".as_bytes()).await {
                defmt::error!("Failed to send hello: {:?}", defmt::Debug2Format(&e));
            }
        }
    }
}

#[embassy_executor::task]
pub async fn start(
    spi_bus: &'static Mutex<CriticalSectionRawMutex, esp_hal::spi::master::Spi<'static, Async>>,
    nss: Output<'static>,
    reset: Output<'static>,
    dio1: Input<'static>,
    busy: Input<'static>,
) {
    defmt::info!("Starting LoRa task");

    let spi_device = embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice::new(spi_bus, nss);
    let mut lora = Lora::new(spi_device, reset, dio1, busy, LoraConfig::default())
        .await
        .unwrap();

    lora.run().await;
}
