![System block diagram](System_Block_Diagram.jpg)

2 solar panels connected to solar charging board CN 3791 which is connected to system (VCC & GND) to [Olimex ESP32-C3 (rev. 1)][olimex-esp32-c3] and the [ESP32-C3 Rust dev kit (tag v1.2)][esp32-c3-rust] by [Espressif][espressif]

The output for the battery on the solar charger is connected a switch to disconnect power going towards the battery and then connected to a [LiPo 3.7V battery of 250 mAh][olimex-battery-250mha] ([Datasheet][battery-datasheet]).

On the connection to the battery, there's also a pin for ADC to the Olimex board for taking measurements of the battery. This allows us to read the battery percentage without charging it through the solar charger or USB (e.g. when we're programming the board).


    Charge voltage 4.2V, nominal voltage 3.7V, cut-off voltage 3.0V
    Recommended charge current 50mA
    Max charge current 125mA
    Recommended discharge current 125mA
    Max discharge current 250mA
    Impedance 60mOhm at 1KHz
    Operating temperature: at charge 0-55C, at discharge -25C+60C
    Capacity loss after 500 cycles full charge/discharge at 20C: 20%
    Dimensions 27x21x5mm


[olimex-esp32-c3]: https://www.olimex.com/Products/IoT/ESP32-C3/ESP32-C3-DevKit-Lipo/
[olimex-battery-250mha]: https://www.olimex.com/Products/Power/Lipo-battery/BATTERY-LIPO250mAh/
[battery-datasheet]: https://www.olimex.com/Products/Power/Lipo-battery/BATTERY-LIPO250mAh/resources/JA602025P-Spec-Data-Sheet-3.7V-250mAh--170116.pdf
[esp32-c3-rust]: https://github.com/esp-rs/esp-rust-board/tree/v1.2
[espressif]: https://www.espressif.com/en/products/devkits