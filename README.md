# ESP32-C3 embedded workshop
# With Embassy and Rust


```
cargo install espflash --rev 60224d1 --git https://github.com/esp-rs/espflash
```


Pinout and schematics of Olimex's ESP32-C3 dev board:

https://raw.githubusercontent.com/OLIMEX/ESP32-C3-DevKit-Lipo/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf

https://github.com/OLIMEX/ESP32-C3-DevKit-Lipo/raw/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf

# Using a Usb-to-serial device for UART debugging

Usi UART0 at 21 (TX) & 20 (RX) GPIO pins.


Run minicom at correct baud rate (`115 200`):
```
minicom -b 115200 --noinit --statline --capturefile=uart_debug.cap --wrap -D /dev/ttyUSB0
```


## License
MIT or APACHE-2.0