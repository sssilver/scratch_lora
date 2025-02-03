use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

use anyhow::{Context, Result};
use esp32_nimble::{
    utilities::mutex::Mutex, uuid128, BLEAdvertisementData, BLEAdvertising, BLECharacteristic,
    BLEDevice, BLEServer, NimbleProperties,
};
use esp_idf_hal::cpu::Core;

use crate::thread;

const DEVICE_NAME: &str = "small-black-box";

type Rx = mpsc::Receiver<Message>;
pub type Tx = mpsc::Sender<Message>;

pub enum Message {
    /// Signal to disconnect the connection
    #[allow(dead_code)]
    Disconnect,

    Notify {
        data: Vec<u8>,
    },
}

pub struct Ble {
    is_connected: Arc<AtomicBool>,
    advertising: &'static Mutex<BLEAdvertising>,
    telemetry_characteristic: Arc<Mutex<BLECharacteristic>>,
}

impl Ble {
    fn new() -> Result<Self> {
        log::info!("initialize");

        // Take the BLE device instance
        let ble_device = BLEDevice::take();
        BLEDevice::set_device_name(DEVICE_NAME).context("failed to set device name")?;

        // Shared connection state
        let is_connected = Arc::new(AtomicBool::new(false));

        // Get BLE advertising and server instances
        let advertising = ble_device.get_advertising();
        let server = ble_device.get_server();

        let service_uuid = uuid128!("17ada41d-b564-4a77-ad1a-22cf554002fc");

        let service = server.create_service(service_uuid);

        let telemetry_characteristic = service.lock().create_characteristic(
            uuid128!("17a8a05b-5da4-44ae-82a5-6d660b08cf13"),
            NimbleProperties::READ | NimbleProperties::NOTIFY,
        );
        telemetry_characteristic.lock().set_value(b"Initial value.");

        advertising.lock().set_data(
            BLEAdvertisementData::new()
                .name(DEVICE_NAME)
                .add_service_uuid(service_uuid),
        )?;

        advertising.lock().start()?;

        // Set up callbacks
        Self::setup_callbacks(server, is_connected.clone());

        Ok(Self {
            is_connected,
            advertising,
            telemetry_characteristic,
        })
    }

    fn setup_callbacks(server: &'static mut BLEServer, is_connected: Arc<AtomicBool>) {
        // Connect callback
        {
            let is_connected = is_connected.clone();
            server.on_connect(move |server, desc| {
                log::info!("connected: {:?}", desc);

                // Update connection state
                is_connected.store(true, Ordering::SeqCst);

                if let Err(e) = server.update_conn_params(desc.conn_handle(), 24, 48, 0, 60) {
                    log::error!("failed to update connection parameters: {:?}", e);
                } else {
                    log::info!("connection parameters updated successfully");
                }
            });
        }

        // Disconnect callback
        {
            let is_connected = is_connected.clone();
            server.on_disconnect(move |desc, reason| {
                log::info!("disconnected: {:?} (reason: {:?})", desc, reason);
                is_connected.store(false, Ordering::SeqCst);
            });
        }
    }

    pub fn run(&self, rx: Rx) -> Result<()> {
        while let Ok(message) = rx.recv() {
            match message {
                Message::Disconnect => self.advertising.lock().stop()?,

                Message::Notify { data } => {
                    if self.is_connected.load(Ordering::SeqCst) {
                        self.telemetry_characteristic
                            .lock()
                            .set_value(&data)
                            .notify();
                        let s = String::from_utf8(data).unwrap();
                        log::info!("notifying of {}", s);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn spawn() -> Result<Tx> {
        let (tx, rx) = mpsc::channel();

        let ble = Ble::new()?;

        let _ = thread::spawn("ble\0", Core::Core0, move || Ok(ble.run(rx)?));

        Ok(tx)
    }
}
