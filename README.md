# ESP32-C3 embedded workshop
## using Embassy, written in Rust


### To setup your environment

Follow the [book](./docs/SUMMARY.md) (Markdown) or run the included [mdbook (Installation)][mdbook-install] book:

[mdbook-install]: https://rust-lang.github.io/mdBook/guide/installation.html

```
git clone git@github.com:LechevSpace/nanosat-workshop.git && \ 
cd nanosat-workshop && \
mdbook serve
```

**NB:** Using `git@github.com:LechevSpace/nanosat-workshop.git` requires SSH key set up on Github.

Use your browser to open [http://localhost:3000](http://localhost:3000)

### How to flash


#### Install `espflash`
```
cargo install espflash --rev 60224d1 --git https://github.com/esp-rs/espflash
```

### `cargo run`


### Pinout and schematics


### Olimex's ESP32-C3 dev board:

https://raw.githubusercontent.com/OLIMEX/ESP32-C3-DevKit-Lipo/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf

https://github.com/OLIMEX/ESP32-C3-DevKit-Lipo/raw/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf


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