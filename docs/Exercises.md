## Power System Application (`power-system` folder)

### Battery and power sense

![Olimex schematic for GPIO 3 and 4](./olimex_battery_and_power_sense.png "Battery (GPIO 3) and power Sense (GPIO4) GPIOs")

## Onboard-computer (`onboard-computer` folder)

### Peripherals

| Peripheral               | Part number                              | Crate                      | Address |
| ------------------------ | ---------------------------------------- | -------------------------- | ------- |
| IMU                      | ICM-42670-P ([Datasheet][IMU-datasheet]) | [icm42670][crate-icm42670] | `0x68`  |
| Temperature and Humidity | SHTC3 ([Datasheet][T-H-datasheet])       | [shtcx][crate-shtcx]       | `0x70`  |

[IMU-datasheet]: https://invensense.tdk.com/download-pdf/icm-42670-p-datasheet/
[crate-icm42670]: https://crates.io/crates/icm42670
[T-H-datasheet]: https://www.mouser.com/datasheet/2/682/Sensirion_04202018_HT_DS_SHTC3_Preliminiary_D2-1323493.pdf
[crate-shtcx]: https://crates.io/crates/shtcx
[onboard-computer-i2c-sensors]: https://github.com/esp-rs/esp-rust-board#i2c-peripherals


## Exercises

Start with the [`power-system`]:

1. Blinky for IO - use the Onboard LED and make it blink
2. Battery measurement with ADC - measure and calculate the battery voltage and percentage
3. Send Battery percentage over UART to the `onboard-computer`

Continue with the next exercises in the `onboard-computer`

4. Receive battery percentage over UART from the `power-system`
5. GNSS receiver - parse NMEA 0183 sentences


## Future ideas you can develop
In no particular order:

- Power sense (`power-system` application)
    By soldering the jumper for `GPIO 4` you can measure the voltage of the +5V Power in line.
    This allows you to know whether or not an external +5V has been provided (both from USB-C or other), that will
    charge the battery.
    Implement another status for the battery which is Charging/External power.
- IMU (`onboard-computer` application)
  Using the **I2C** peripheral and the included IMU sensor on the `onboard-computer`, take readings of the 
- Humidity and Temperature sensor (`onboard-computer` application)

