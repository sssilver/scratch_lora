# Prototype V3 Field Test

## Firmware
The test firmware on both test prototypes does the following:

- Upon startup, it starts a **millisecond** timer
- LoRa is configured for:
    - Spread factor: SF12
    - Bandwidth: 125kHz
    - Coding rate: 4/8
    - Power: 20dBm (set to maximum power)
    - Frequency: Decide between 150.00 MHz (native module frequency), or 151.82 MHz (lowest MURS frequency)
- Every second, it switches LoRa to TX mode and broadcasts a **32-byte** message via LoRa, containing the string `ABCDEFGHIJKLMNOPQRSTUVWXYZ012345`, then switches back to RX mode
- Every time it broadcasts, it also writes the following message into the serial port, as well as appends it into a log file:
    - `T,<timer>,<lat>,<long>,<altitude>,<rssi>\n`
    - Note that the _transmitted_ message is not logged
- The rest of the time, the LoRa is being monitored in TX mode, and every time a message is received:
    1. The following is logged into both serial and  the file:
        - `R,<timer>,<lat>,<long>,<altitude>,<rssi>,<MESSAGE>\n`
    2. LED is flashed for 100ms


### Important!
The radio operates in half-duplex mode, meaning that it can only receive or transmit, but not both at the same time. This means that the transmission phase of prototype A must be shifted from the transmission phase of prototype B, otherwise they will both be transmitting at the same time, and fail to receive each other's messages.

Read the test plan to understand how to avoid this.

## Test Plan
1. Prepare two laptops that have enough battery life to last for the entire test, _with this test plan opened on both laptops_
2. Connect each prototype to a laptop and launch serial monitors
3. **IMPORTANT:** Check *both* serial monitors to ensure that:
    1. The devices are working
    2. **The devices are not transmitting at the same time!**
    3. The startup timers are within **< 1000 milliseconds** of each other
4. Move each laptop to its respective vehicle
5. Each vehicle drives to its next respective destination while the laptop is on and serial is monitored
    - If anything unusual is observed in the serial monitor, Sergey and Areg should be notified
6. Once both vehicles confirm with each other that they have reached their respective destinations, they move on to the next destination
7. This process repeats until all destinations are traversed

## Routes
Once both vehicles arrive at each milestone, and both drivers confirm that they're ready to depart for the next milestone, both vehicle operators should fill in their respective table.

### Vehicle A
1. [40.17764, 44.51255](https://www.openstreetmap.org/#map=16/40.17764/44.51255) Republic square 
2. [40.17764, 44.51255](https://www.openstreetmap.org/#map=16/40.17764/44.51255) Wait in republic square and monitor serial
3. [40.18779, 44.60947](https://www.openstreetmap.org/#map=14/40.18779/44.60947) Drive to Jrvezh
4. [40.22421, 44.45249](https://www.openstreetmap.org/#map=15/40.22421/44.45249) Drive to Vahagni

```
----------------------------------------------------------------------
# | GPS Coordinates    | Time | LED | Serial healthy? (Explain if not)
----------------------------------------------------------------------
1 | 40.17764, 44.51255 |      | [ ] |
2 | 40.17764, 44.51255 |      | [ ] |
3 | 40.18779, 44.60947 |      | [ ] |
4 | 40.22421, 44.45249 |      | [ ] |
----------------------------------------------------------------------
```

### Vehicle B
1. [40.17764, 44.51255](https://www.openstreetmap.org/#map=16/40.17764/44.51255) Republic square
2. [40.18779, 44.60947](https://www.openstreetmap.org/#map=14/40.18779/44.60947) Drive to Jrvezh
3. [40.12232, 44.74131](https://www.openstreetmap.org/#map=13/40.12232/44.74131) Drive towards Garni
    * Stop and turn back once there's no signal for more than 60 seconds
4. [40.28889, 44.38434](https://www.openstreetmap.org/#map=14/40.28889/44.38434) Drive towards Ashtarak
    * Stop and turn back once there's no signal for more than 60 seconds

```
----------------------------------------------------------------------
# | GPS Coordinates    | Time | LED | Serial healthy? (Explain if not)
----------------------------------------------------------------------
1 | 40.17764, 44.51255 |      | [ ] |
2 | 40.18779, 44.60947 |      | [ ] |
3 | 40.12232, 44.74131 |      | [ ] |
4 | 40.28889, 44.38434 |      | [ ] |
----------------------------------------------------------------------
```
