# Nanosat embedded workshop
## using Embassy, written in Rust


### Getting started

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