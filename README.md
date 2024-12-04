# Nanosat embedded workshop
## using Embassy, written in Rust

### Going through the Workshop

Using [`cargo-generate` (install instructions)](https://github.com/cargo-generate/cargo-generate/https://github.com/cargo-generate/cargo-generate/?tab=readme-ov-file#installation) make sure to generate a new project using the `skeleton` branch where a skeleton application with notes about the exercises will be found:

```
# with ssh
cargo generate --branch skeleton git@github.com:AeroRust/nanosat-workshop.git
# with https
cargo generate --branch skeleton https://github.com/AeroRust/nanosat-workshop.git
```

### ⚠️ Important: The state of the firmware is very unstable

### Structure

- [`website` aerorust.org/nanosat-workshop/](https://aerorust.org/nanosat-workshop/) - The workshop setup, information and exercises.

- [onboard-computer](./onboard-computer/)
    - [`protocol`](./onboard-computer/protocol) - common structures for use in firmware and on a host machine when, for example, you are building a CLI.
    - [`pico-w`](./onboard-computer/pico-w) - embassy application build for running on a RP Pico W - **most up to date firmware***
    - [`esp32c3`](./onboard-computer/esp32c3) - embassy application build for running on ESP32-c3 Rust development board from Espressif systems (currently unsupported)
- power-system - `esp32-c3` based on Olimex's ESP32C3 dev. board - currently unsupported.
- [`lc76`](./lc76g/) - Protocol implementation for the GNSS receiver LC76G UART used in the project for settings structures which are separate from the received `nmea` sentences.
- [`cyw43-firmware`](./cyw43-firmware/) - RP Pico W WiFi chip firmware, taken from embassy repo.

In each project you might find aliases defined in `.cargo/config.toml` file which setup various features, flags and run different applications.
Make sure to check them out as they will ease out your development process when building and flashing your firmware.

### Development
#### Getting started

To setup your environment and learn about the exercises follow the book.
You can access the book in a few ways:

- On https://aerorust.org/nanosat-workshop/
- Using the [Markdown](./docs/SUMMARY.md) (links pointing to the documentation of items in the project will not work)
- Run the included `mdbook` [(Installation)][mdbook-install] book:

[mdbook-install]: https://rust-lang.github.io/mdBook/guide/installation.html

1. Clone the repo
```
git clone git@github.com:AeroRust/nanosat-workshop.git && \ 
cd nanosat-workshop
```

**NB:** Using `git@github.com:AeroRust/nanosat-workshop.git` requires SSH key set up on Github.

2. Build the docs of the applications

`cargo +nightly website-docs`

3. Start the local book server
`mdbook serve`


1. Use your browser to open http://localhost:3000

### How to flash

#### 1. Install `espflash`

```
cargo install espflash@2.0.0-rc.4
```

#### 2. Use `cargo run`

- For `power-system` application (Olimex board) `cargo run -p power-system`
- For `onboard-computer` application (Espressif Rust board) `cargo run -p onboard-computer`


### Pinout and schematics


##### Olimex's ESP32-C3 dev board:

PDF: https://raw.githubusercontent.com/OLIMEX/ESP32-C3-DevKit-Lipo/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf

Repository PDF file: https://github.com/OLIMEX/ESP32-C3-DevKit-Lipo/raw/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf


##### Espressif's ESP32-C3-DevKit-RUST-1

Pinout, docs, schematics, etc. can be found here: https://github.com/esp-rs/esp-rust-board

### Debugging with UART

#### Using a Usb-to-serial device for UART debugging

Use UART0 at 21 (TX) & 20 (RX) GPIO pins.


Run minicom at correct baud rate (`115 200`):
```
minicom -b 115200 --noinit --statline --capturefile=uart_debug.cap --wrap -D /dev/ttyUSB0
```

This will save all caught data in a file called `uart_debug.cap`.


## License

MIT or APACHE-2.0