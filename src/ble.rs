use embassy_futures::join::join;
use embassy_futures::select::select;
use embassy_time::Timer;
use trouble_host::prelude::*;

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

// GATT Server definition
#[gatt_server]
struct Server {
    battery_service: BatteryService,
}

/// Battery service
#[gatt_service(uuid = "17ada41d-b564-4a77-ad1a-22cf554002fc")] // Use the custom UUID that worked
struct BatteryService {
    /// Battery Level
    #[descriptor(uuid = descriptors::VALID_RANGE, read, value = [0, 100])]
    #[descriptor(uuid = descriptors::MEASUREMENT_DESCRIPTION, read, value = "Battery Level")]
    #[characteristic(uuid = characteristic::BATTERY_LEVEL, read, notify, value = 10)]
    level: u8,
    #[characteristic(uuid = "17a8a05b-5da4-44ae-82a5-6d660b08cf13", write, read, notify)]
    status: bool,
}

/// Run the BLE stack.
pub async fn run<C, const L2CAP_MTU: usize>(controller: C)
where
    C: Controller,
{
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    defmt::info!("Our address = {:?}", defmt::Debug2Format(&address));

    let mut resources: HostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    defmt::info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "small-black-box",
        appearance: &appearance::outdoor_sports_activity::LOCATION_AND_NAVIGATION_POD,
    }))
    .unwrap();

    let _ = join(ble_task(runner), async {
        loop {
            match advertise("small-black-box", &mut peripheral).await {
                Ok(conn) => {
                    // set up tasks when the connection is established to a central, so they don't run when no one is connected.
                    let a = gatt_events_task(&server, &conn);
                    let b = custom_task(&server, &conn, &stack);
                    // run until any task ends (usually because the connection has been closed),
                    // then return to advertising state.
                    select(a, b).await;
                }
                Err(e) => {
                    let e = defmt::Debug2Format(&e);
                    panic!("[adv] error: {:?}", e);
                }
            }
        }
    })
    .await;
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
///
/// ## Alternative
///
/// If you didn't require this to be generic for your application, you could statically spawn this with i.e.
///
/// ```rust,ignore
///
/// #[embassy_executor::task]
/// async fn ble_task(mut runner: Runner<'static, SoftdeviceController<'static>>) {
///     runner.run().await;
/// }
///
/// spawner.must_spawn(ble_task(runner));
/// ```
async fn ble_task<C: Controller>(mut runner: Runner<'_, C>) {
    loop {
        if let Err(e) = runner.run().await {
            let e = defmt::Debug2Format(&e);
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

/// Stream Events until the connection closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
async fn gatt_events_task(server: &Server<'_>, conn: &Connection<'_>) -> Result<(), Error> {
    let level = server.battery_service.level;
    loop {
        match conn.next().await {
            ConnectionEvent::Disconnected { reason } => {
                defmt::info!("[gatt] disconnected: {:?}", defmt::Debug2Format(&reason));
                break;
            }
            ConnectionEvent::Gatt { data } => {
                // We can choose to handle event directly without an attribute table
                // let req = data.request();
                // ..
                // data.reply(conn, Ok(AttRsp::Error { .. }))

                // But to simplify things, process it in the GATT server that handles
                // the protocol details
                match data.process(server).await {
                    // Server processing emits
                    Ok(Some(event)) => {
                        match &event {
                            GattEvent::Read(event) => {
                                if event.handle() == level.handle {
                                    let value = server.get(&level);
                                    defmt::info!(
                                        "[gatt] Read Event to Level Characteristic: {:?}",
                                        defmt::Debug2Format(&value)
                                    );
                                }
                            }
                            GattEvent::Write(event) => {
                                if event.handle() == level.handle {
                                    defmt::info!(
                                        "[gatt] Write Event to Level Characteristic: {:?}",
                                        event.data()
                                    );
                                }
                            }
                        }

                        // This step is also performed at drop(), but writing it explicitly is necessary
                        // in order to ensure reply is sent.
                        match event.accept() {
                            Ok(reply) => {
                                reply.send().await;
                            }
                            Err(e) => {
                                defmt::warn!(
                                    "[gatt] error sending response: {:?}",
                                    defmt::Debug2Format(&e)
                                );
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        defmt::warn!(
                            "[gatt] error processing event: {:?}",
                            defmt::Debug2Format(&e)
                        );
                    }
                }
            }
        }
    }
    defmt::info!("[gatt] task finished");
    Ok(())
}

async fn advertise<'a, C: Controller>(
    name: &'a str,
    peripheral: &mut Peripheral<'a, C>,
) -> Result<Connection<'a>, BleHostError<C::Error>> {
    defmt::info!("Starting advertise function");

    let mut advertiser_data = [0; 31];
    defmt::info!("Encoding advertisement data");

    let _ = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;

    defmt::info!("About to call peripheral.advertise()");
    match peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..],
                scan_data: &[],
            },
        )
        .await
    {
        Ok(advertiser) => {
            defmt::info!("Advertisement started successfully");
            let conn = advertiser.accept().await?;
            defmt::info!("[adv] connection established");
            Ok(conn)
        }
        Err(e) => {
            defmt::error!("Failed to start advertising: {:?}", defmt::Debug2Format(&e));
            Err(e)
        }
    }
}

/// Example task to use the BLE notifier interface.
/// This task will notify the connected central of a counter value every 2 seconds.
/// It will also read the RSSI value every 2 seconds.
/// and will stop when the connection is closed by the central or an error occurs.
async fn custom_task<C: Controller>(
    server: &Server<'_>,
    conn: &Connection<'_>,
    stack: &Stack<'_, C>,
) {
    let mut tick: u8 = 0;
    let level = server.battery_service.level;
    loop {
        tick = tick.wrapping_add(1);
        defmt::info!("[custom_task] notifying connection of tick {}", tick);
        if level.notify(server, conn, &tick).await.is_err() {
            defmt::info!("[custom_task] error notifying connection");
            break;
        };
        // read RSSI (Received Signal Strength Indicator) of the connection.
        if let Ok(rssi) = conn.rssi(stack).await {
            defmt::info!("[custom_task] RSSI: {:?}", rssi);
        } else {
            defmt::info!("[custom_task] error getting RSSI");
            break;
        };
        Timer::after_secs(2).await;
    }
}
