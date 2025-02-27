use bt_hci::controller::ExternalController;
use config::{Config, Resources, DEVICE_SERVICE_UUID};
use embassy_futures::{join::join, select::select};
use embassy_time::Timer;
use error::Error;
use esp_hal::peripherals::BT;
use esp_wifi::{ble::controller::BleConnector, EspWifiController};
use service::DeviceService;
use state::StateController;
use trouble_host::prelude::*;

mod config;
mod error;
mod service;
pub mod state;

/// BLE stack and its connection state
pub struct Ble<'a, C: Controller> {
    config: Config,
    peripheral: Peripheral<'a, C>,
    server: Server<'a>,
    state_controller: StateController,
}

#[gatt_server]
pub struct Server {
    device_service: DeviceService,
}

impl<'a, C: Controller> Ble<'a, C> {
    /// Create a new BLE instance
    ///
    /// * `peripheral` - The BLE peripheral interface
    /// * `stack` - Reference to the BLE stack
    /// * `config` - BLE configuration parameters
    fn new(peripheral: Peripheral<'a, C>, config: Config) -> Result<Self, Error> {
        let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
            name: config.name,
            appearance: &appearance::outdoor_sports_activity::LOCATION_AND_NAVIGATION_POD,
        }))
        .map_err(|_| Error::GattError)?;

        let state_controller = StateController::new();

        Ok(Self {
            peripheral,
            server,
            config,
            state_controller,
        })
    }

    /// Start the BLE service and handles connections asynchronously
    ///
    /// * `stack` - Reference to the BLE stack
    /// * `config` - BLE configuration parameters
    ///
    /// # Returns
    /// A reference to the BLE state watch channel

    async fn start(stack: &'a Stack<'a, C>, config: Config) -> Result<(), Error> {
        let Host {
            peripheral, runner, ..
        } = stack.build();

        let mut ble = Self::new(peripheral, config)?;

        join(
            ble_task(runner),
            async move { ble.run_connection_loop().await },
        )
        .await;

        Ok(())
    }

    /// Run the main BLE connection loop, handling advertising and events
    async fn run_connection_loop(&mut self) {
        loop {
            embassy_futures::yield_now().await;

            match advertise(self.config.name, &mut self.peripheral).await {
                Ok(conn) => {
                    defmt::info!("BLE connected");
                    self.state_controller.set_connected();

                    // Run all connection-dependent tasks
                    select(
                        // BLE tasks
                        self.gatt_events_task(&conn),
                        self.telemetry_task(&conn),
                    )
                    .await;

                    // Handle disconnection regardless of which task exited
                    defmt::info!("BLE disconnected");
                    self.state_controller.set_disconnected();
                }
                Err(_) => {
                    defmt::error!("Error establishing a BLE connection");
                    self.state_controller.set_disconnected();
                    Timer::after_secs(1).await;
                }
            }
        }
    }

    /// Handle GATT events for the BLE server
    async fn gatt_events_task(&self, conn: &Connection<'_>) -> Result<(), Error> {
        let level = &self.server.device_service.status;
        loop {
            embassy_futures::yield_now().await;

            match conn.next().await {
                ConnectionEvent::Disconnected { reason: _ } => break,
                ConnectionEvent::Gatt { data } => match data.process(&self.server).await {
                    Ok(Some(event)) => {
                        match &event {
                            GattEvent::Read(event) => {
                                if event.handle() == level.handle {
                                    let _value = self.server.get(&level);
                                }
                            }
                            GattEvent::Write(event) => if event.handle() == level.handle {},
                        }
                        if let Ok(reply) = event.accept() {
                            reply.send().await;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                },
            }
        }
        Ok(())
    }

    async fn telemetry_task(&self, conn: &Connection<'_>) -> Result<(), Error> {
        let mut counter: u8 = 0;
        let status = self.server.device_service.status;

        loop {
            counter = counter.wrapping_add(1);

            if status.notify(&self.server, conn, &counter).await.is_err() {
                break;
            }

            defmt::info!("Counter: {}", counter);
            Timer::after_secs(1).await;
        }
        Ok(())
    }
}

/// Run the BLE host stack task
async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) {
    loop {
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }

        embassy_futures::yield_now().await;
    }
}

/// Advertise the BLE device for incoming connections
async fn advertise<'a, C: Controller>(
    name: &'a str,
    peripheral: &mut Peripheral<'a, C>,
) -> Result<Connection<'a>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];

    let adv_len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids128(&[DEVICE_SERVICE_UUID.into()]),
        ],
        &mut advertiser_data[..],
    )?;

    let mut scan_data = [0; 31];
    let scan_len = AdStructure::encode_slice(
        &[AdStructure::ShortenedLocalName(name.as_bytes())],
        &mut scan_data[..],
    )?;

    match peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..adv_len],
                scan_data: &scan_data[..scan_len],
            },
        )
        .await
    {
        Ok(advertiser) => {
            embassy_futures::yield_now().await;

            let conn = advertiser.accept().await?;
            Ok(conn)
        }
        Err(e) => Err(e),
    }
}

/// Initialize and start the BLE module (entry point for the BLE module)
#[embassy_executor::task]
pub async fn start(bt: BT, init: EspWifiController<'static>) {
    defmt::info!("starting BLE");
    let connector = BleConnector::new(&init, bt);

    let controller: ExternalController<_, 20> = ExternalController::new(connector);

    let mut resources = Resources::new();

    let config = Config::default();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(config.address);

    Ble::start(&stack, config).await.unwrap();
}
