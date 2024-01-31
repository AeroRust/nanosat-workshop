## Board's applications

Both applications are structured in the following way to allow you to follow the exercises:
- `Application::init` - initialise any peripheral that will be used for the `Application`
  - [`power_system::Application::init`][power-system-application-init]
  - [`onboard_computer::Application::init`][onboard-computer-application-init]

[power-system-application-init]: ./skeleton/riscv32imc-unknown-none-elf/doc/power_system/application/struct.Application.html#method.init
[onboard-computer-application-init]: ./skeleton/riscv32imc-unknown-none-elf/doc/onboard_computer/application/struct.Application.html#method.init

- `Application:run` - spawns all tasks on the embassy executor needed for our `Application`.
  - [`power_system::Application::run`][power-system-application-run]
  - [`onboard_computer::Application::run`][onboard-computer-application-run]
  - `power_system::Application::init`
- An embassy task `fn run_*(..)` for each exercise

[power-system-application-run]: ./skeleton/riscv32imc-unknown-none-elf/doc/power_system/application/struct.Application.html#method.run
[onboard-computer-application-run]: ./skeleton/riscv32imc-unknown-none-elf/doc/onboard_computer/application/struct.Application.html#method.run

### Power System Application (`power-system` folder)

#### Battery and power sense

![Olimex schematic for GPIO 3 and 4](./olimex_battery_and_power_sense.png "Battery (GPIO 3) and power Sense (GPIO4) GPIOs")

### Onboard-computer (`onboard-computer` folder)

#### Peripherals

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

Start with the [`power-system`][power-system]:

1. Flashing Onboard LED - a blinky example for IO - [`power_system::application::run_blinky`][ps-run_blinky]
2. Battery measurement with ADC - measure and calculate the battery voltage and percentage - [`power_system::application::run_battery_measurement_adc`][ps-run_battery_measurement_adc]
3. Send Battery percentage over UART to the `onboard-computer` - [`power_system::application::run_uart`][ps-run_uart]

[ps-run_blinky]: ./skeleton/riscv32imc-unknown-none-elf/doc/power_system/application/fn.run_blinky.html
[ps-run_battery_measurement_adc]: ./skeleton/riscv32imc-unknown-none-elf/doc/power_system/application/fn.run_battery_measurement_adc.html
[ps-run_uart]: ./skeleton/riscv32imc-unknown-none-elf/doc/power_system/application/fn.run_uart.html

Continue with the next exercises in the [`onboard-computer`][onboard-computer]:

1. Receive battery percentage over UART from the `power-system` - [`onboard_computer::application::run_uart`][obc-run_uart]
2. GNSS receiver - parse NMEA 0183 sentences - [`onboard_computer::application::run_gnss`][obc-run_gnss]

[power-system]: ./skeleton/riscv32imc-unknown-none-elf/doc/power_system/index.html
[onboard-computer]: ./skeleton/riscv32imc-unknown-none-elf/doc/onboard_computer/index.html
[obc-run_uart]: ./skeleton/riscv32imc-unknown-none-elf/doc/onboard_computer/application/fn.run_uart.html
[obc-run_gnss]: ./skeleton/riscv32imc-unknown-none-elf/doc/onboard_computer/application/fn.run_gnss.html

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

