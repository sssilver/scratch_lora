use crate::{
    ble::state::{BleStateRx, BLE_STATE},
    gnss::{positioning::GnssPositioning, watch::GnssStateRx, watch::GNSS_WATCH},
};
use core::fmt::Write;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Timer};
use embedded_graphics::prelude::Point;
use heapless::String;

use super::DisplayDevice;

pub struct DisplayController {
    display: DisplayDevice<'static>,

    ble_rx: BleStateRx,
    gps_rx: GnssStateRx,

    is_ble_connected: bool,
    positioning: Option<GnssPositioning>,

    last_update: Option<embassy_time::Instant>,
}

impl DisplayController {
    pub fn new(display: DisplayDevice<'static>, ble_rx: BleStateRx, gps_rx: GnssStateRx) -> Self {
        Self {
            display,
            ble_rx,
            gps_rx,
            is_ble_connected: false,
            positioning: None,
            last_update: None,
        }
    }

    fn update_display(&mut self) -> Result<(), &'static str> {
        self.display.clear().unwrap();

        // BLE status
        let mut ble_status: String<16> = String::new();
        write!(
            &mut ble_status,
            "[{}] BLE",
            if self.is_ble_connected { "X" } else { " " }
        )
        .unwrap_or_default();
        self.display.draw_text(&ble_status, Point::zero()).unwrap();

        // GPS status
        let mut gps_status_latitude: String<64> = String::new();
        let mut gps_status_longitude: String<64> = String::new();
        if let Some(position) = &self.positioning {
            write!(&mut gps_status_latitude, "{}", position.latitude).unwrap_or_default();
            write!(&mut gps_status_longitude, "{}", position.longitude).unwrap_or_default();
        } else {
            write!(&mut gps_status_latitude, "No GPS fix").unwrap_or_default();
            write!(&mut gps_status_longitude, "").unwrap_or_default();
        }
        self.display
            .draw_text(&gps_status_latitude, Point::new(0, 16))
            .unwrap();

        self.display
            .draw_text(&gps_status_longitude, Point::new(0, 32))
            .unwrap();

        // Additional status info
        let mut update_time: String<32> = String::new();
        if let Some(instant) = self.last_update {
            write!(
                &mut update_time,
                "Updated: {}ms ago",
                instant.elapsed().as_millis()
            )
            .unwrap_or_default();
            self.display
                .draw_text(&update_time, Point::new(0, 48))
                .unwrap();
        }

        Ok(())
    }

    pub async fn run(mut self) {
        // Initial display update
        if let Err(e) = self.update_display() {
            defmt::error!("Display error on startup: {:?}", e);
        }
        self.last_update = Some(embassy_time::Instant::now());

        // Force update every 30 seconds no matter what
        const FORCED_UPDATE_INTERVAL: Duration = Duration::from_secs(30);
        let mut force_update_timer = Timer::after(FORCED_UPDATE_INTERVAL);

        loop {
            let state_change = select(
                select(self.ble_rx.changed(), self.gps_rx.changed()),
                &mut force_update_timer,
            );

            match state_change.await {
                // Either BLE or GPS state changed
                Either::First(either) => {
                    let mut should_update_display = false;

                    match either {
                        Either::First(_) => {
                            // BLE state changed
                            if let Some(ble_state) = self.ble_rx.try_get() {
                                if ble_state.connection_status != self.is_ble_connected {
                                    defmt::info!(
                                        "BLE connection status changed: {}",
                                        ble_state.connection_status
                                    );
                                    self.is_ble_connected = ble_state.connection_status;
                                    should_update_display = true;
                                }
                            }
                        }
                        Either::Second(_) => {
                            // GPS state changed
                            if let Some(gps_state) = self.gps_rx.try_get() {
                                if self.positioning != gps_state {
                                    defmt::info!(
                                        "GPS position updated: {:?}",
                                        defmt::Debug2Format(&gps_state)
                                    );
                                    self.positioning = gps_state;
                                    should_update_display = true;
                                }
                            }
                        }
                    }

                    if should_update_display {
                        if let Err(e) = self.update_display() {
                            defmt::error!("Display update error: {:?}", e);
                        } else {
                            self.last_update = Some(embassy_time::Instant::now());
                            // Reset the force update timer after a successful update
                            force_update_timer = Timer::after(FORCED_UPDATE_INTERVAL);
                        }
                    }
                }
                // Forced update timer elapsed
                Either::Second(_) => {
                    defmt::debug!("Forced display update timer elapsed");
                    if let Err(e) = self.update_display() {
                        defmt::error!("Display update error during forced update: {:?}", e);
                    } else {
                        self.last_update = Some(embassy_time::Instant::now());
                    }
                    // Restart the force update timer
                    force_update_timer = Timer::after(FORCED_UPDATE_INTERVAL);
                }
            }

            // Short delay to prevent excessive CPU usage if many state changes happen
            Timer::after_millis(50).await;
        }
    }
}

#[embassy_executor::task]
pub async fn start(mut display: DisplayDevice<'static>) {
    defmt::info!("Starting display controller");

    match (BLE_STATE.receiver(), GNSS_WATCH.receiver()) {
        (Some(ble_rx), Some(gps_rx)) => {
            let display_controller = DisplayController::new(display, ble_rx, gps_rx);

            display_controller.run().await;
        }
        _ => {
            defmt::error!("Failed to get BLE or GPS receiver");

            if let Ok(()) = display.clear() {
                let _ = display.draw_text("STATE ERROR", Point::zero());
            }

            return;
        }
    }
}
