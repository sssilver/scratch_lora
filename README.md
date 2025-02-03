# Box Prototype v3

This is the third prototype of the Small Black Box firmware, and it's the first one that's built using Rust.

## Hardware

The firmware is built for the ESP32-S3-WROOM-2 microcontroller. The target for the Cargo toolchain is `esp32s3`. To run the firmware, an ESP32-S3 microcontroller is required with support for BLE and a built-in flash memory.

The firmware also assumes the presence of an SX127x LoRa module and a GPS module (model to be determined).

## Development Environment

The toolchain for running this prototype is run in a Docker container, and is part of the rest of the Visor dev stack.

To flash the firmware, you'll need to install `espflash`:
```
cargo binstall espflash
```

## Building the firmware

Once the prerequisites are installed, building the firmware is done through `cargo watch` that comes up as part of the main dev stack docker-compose stack.

## Flushing the firmware to the ESP32

Simply building the firmware may be satisfying, but it's not very useful. To actually run the firmware on the ESP32, it'll need to be flashed to the hardware:

```
espflash flash
```
Or if you'd like to immediately start monitoring the output:
```
espflash flash --monitor
```

## Monitoring the output

To monitor the output of the firmware, use:

```
espflash monitor
```
