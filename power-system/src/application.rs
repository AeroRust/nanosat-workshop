use embassy_executor::Executor;

use embassy_time::{Duration, Timer};

use esp_println::println;

use hal::{
    adc::{AdcConfig, AdcPin, Attenuation, ADC, ADC1},
    clock::ClockControl,
    embassy,
    gpio::{Analog, Gpio3, Gpio8, Output, PushPull},
    interrupt,
    peripherals::{Interrupt, Peripherals, UART1},
    prelude::*,
    system::SystemParts,
    timer::TimerGroup,
    uart::{
        TxRxPins,
    },
    Priority, Rtc, Uart, IO,
};

use core::{
    fmt::Write,
    sync::atomic::{AtomicU8},
    writeln,
};

pub static BATTERY: AtomicU8 = AtomicU8::new(0);

/// # Exercise: Flashing Onboard LED
///
/// Read docs: <https://docs.rs/esp32-hal/latest/esp32_hal/gpio/index.html>
/// Check out the Blinky example: <https://github.com/esp-rs/esp-hal/blob/main/esp32c3-hal/examples/blinky.rs>
///
/// Check out Olimex schematic, for pins and other board features: <https://raw.githubusercontent.com/OLIMEX/ESP32-C3-DevKit-Lipo/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf>
///
/// We want to create an Output of GPIO 8 which is the onboard led (check schematics above).
///
/// GPIO 8 docs: <https://docs.rs/esp32c3-hal/latest/esp32c3_hal/gpio/type.Gpio8.html>
///
///
///
/// `pub type OnboardLed = todo!("");`
pub type OnboardLed = Gpio8<Output<PushPull>>;

/// # Exercise: Battery measurement with ADC
///
/// - GPIO 3 docs: <https://docs.rs/esp32c3-hal/latest/esp32c3_hal/gpio/type.Gpio3.html>
/// - ADC example: <https://github.com/esp-rs/esp-hal/blob/main/esp32c3-hal/examples/adc.rs>
pub type BatteryMeasurementPin = AdcPin<Gpio3<Analog>, ADC1>;

pub struct Application {
    adc: ADC<'static, ADC1>,
    uart: Uart<'static, UART1>,
    onboard_led: OnboardLed,
    battery_measurement_pin: BatteryMeasurementPin,
}

impl Application {
    /// Initialises all the peripherals which the [`Application`] will use.
    pub fn init(peripherals: Peripherals) -> Self {
        let system: SystemParts = peripherals.SYSTEM.split();
        let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

        let mut rtc = Rtc::new(peripherals.RTC_CNTL);
        let mut peripheral_clock_control = system.peripheral_clock_control;
        let timer_group0 =
            TimerGroup::new(peripherals.TIMG0, &clocks, &mut peripheral_clock_control);
        let mut wdt0 = timer_group0.wdt;
        let timer_group1 =
            TimerGroup::new(peripherals.TIMG1, &clocks, &mut peripheral_clock_control);
        let mut wdt1 = timer_group1.wdt;

        // Disable watchdog timers
        rtc.swd.disable();
        rtc.rwdt.disable();
        wdt0.disable();
        wdt1.disable();

        #[cfg(feature = "embassy-time-systick")]
        embassy::init(&clocks, system_timer);

        #[cfg(feature = "embassy-time-timg0")]
        embassy::init(&clocks, timer_group0.timer0);

        // Setup IO peripherals for application
        //
        // TODO: Uncomment line and implement the `todo!()` for all exercises requiring GPIO.
        // let io = todo!("Setup the GPIO peripherals");
        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        // Exercise: Battery measurement
        //
        // Set GPIO 3 as a battery measurement pin but keep it mind that it's not mandatory to make an external voltage divider.
        // instead you can solder 3 jumpers on the board and use the integrated voltage divider of the Olimex board at GPIO 3.
        // GPIO 4 can be used to sense the power flowing into the board at +5V when e.g. the USB is connected to the board.
        // This corresponds to a "Charging" or "Not charging" state of the battery.
        // TODO: Can we use an `Input` GPIO instead of ADC measurement?
        //
        // Olimex ESP32-C3 schematics: https://raw.githubusercontent.com/OLIMEX/ESP32-C3-DevKit-Lipo/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf
        //
        // Create an ADC instances
        // HAL example: https://github.com/esp-rs/esp-hal/blob/main/esp32c3-hal/examples/adc.rs
        //
        // 1. Get the ADC peripherals
        // TODO: Uncomment line and implement the `todo!()` for all exercises requiring ADC.
        // let analog = todo!("You need to split() the `APB_SARADC` peripheral from `peripherals`");
        let analog = peripherals.APB_SARADC.split();
        // 2. Create a configuration for ADC1
        //
        // TODO: Uncomment line and implement the `todo!()` for all exercises requiring ADC1.
        // let adc1_config = todo!("Create an AdcConfig instance for ADC1");
        let mut adc1_config = AdcConfig::new();
        // 3. Create the battery measurement pin by enabling it as an Analog pin
        //
        // Attenuation 11db should be used to get measurements between 0 and 2500 mV (or 2.5 V).
        // Given that we use a voltage divider and the maximum (the 100% of the battery) is 4.2 V,
        // this gives a range of `4.2V / 2 = 2.1V`
        //
        // TODO: Uncomment line and implement the `todo!()` for all exercises requiring ADC1.
        // let battery_measurement_pin = todo!("Enable GPIO 3 as analog and Attenuation - 11dB for measuring the battery voltage");
        let battery_measurement_pin =
            adc1_config.enable_pin(io.pins.gpio3.into_analog(), Attenuation::Attenuation11dB);

        // 4. Initialise ADC1 peripheral
        let adc1 = ADC::<ADC1>::adc(&mut peripheral_clock_control, analog.adc1, adc1_config)
            .expect("Failed to init ADC1");

        // TODO: Upcoming future exercise - power sensing:
        // adc1_config.enable_pin(io.pins.gpio4.into_analog(), Attenuation::Attenuation11dB);

        // Exercise: Flashing Onboard LED
        //
        // Olimex ESP32-C3 schematics: https://raw.githubusercontent.com/OLIMEX/ESP32-C3-DevKit-Lipo/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf
        // Configure GPIO and set GPIO 8 (LED pin) as an Push/Pull output pin
        // TODO: Uncomment line and implement the `todo!()` for the exercise
        // let onboard_led = todo!("Onboard Led setup - Push/Pull Output pin")
        let onboard_led = io.pins.gpio8.into_push_pull_output();

        // Exercise: Send battery percentage to `onboard-computer` over UART.
        //
        // The Olimex ESP32-C3 board has debug UART on pins 21 (TX) and 20 (RX)
        // but you should use GPIO pins 0 (TX) and 1 (RX) for sending and receiving data respectively
        // to/from the `onboard-computer` board.
        //
        // 1. Setup a new Uart instance using the UART1 peripheral and set the pins in the configuration
        //
        // TODO: Uncomment line and implement the `todo!()` for the exercise
        // let uart1 = todo!("Configure UART 1 at pins 0 (TX) and 1 (RX) with `None` or default for the `Config`");
        let uart1 = Uart::new_with_config(
            peripherals.UART1,
            None,
            Some(TxRxPins::new_tx_rx(
                io.pins.gpio0.into_floating_input(),
                io.pins.gpio1.into_push_pull_output(),
            )),
            &clocks,
            &mut peripheral_clock_control,
        );
        // 2. Enable the UART1 interrupt using Priority1
        // HAL docs: https://docs.rs/esp32c3-hal/latest/esp32c3_hal/interrupt/fn.enable.html
        //
        // TODO: Uncomment line, import the `interrupt` module and enable UART1 with Priority1
        // Caveat: Do not forget to handle the result!
        //
        // interrupt::enable(...)
        interrupt::enable(Interrupt::UART1, Priority::Priority1).unwrap();

        Self {
            adc: adc1,
            uart: uart1,
            onboard_led,
            battery_measurement_pin,
        }
    }


    /// Runs the application by spawning each of the [`Application`]'s tasks
    pub fn run(self, executor: &'static mut Executor) -> ! {
        executor.run(|spawner| {
            spawner.must_spawn(run_battery_measurement_adc(
                self.adc,
                self.battery_measurement_pin,
            ));
            spawner.must_spawn(run_blinky(self.onboard_led));
            spawner.must_spawn(run_uart(self.uart));
        })
    }
}

// UART Transmit Communication Task
#[embassy_executor::task]
async fn run_uart(mut uart: Uart<'static, UART1>) {
    // This communication task will be executed every 1 second
    // First the battery percentage will be sent to the power system
    // Second, the task will enter blocking state until new GNSS message is received from power system
    loop {
        // Transmit Operations
        // First we send the battery percentage value to the Power system
        uart.write_str("Hello world!").ok();

        // Receive Operations
        // Second we poll UART receiver to check if any messages are received
        // This code will receive NMEA messages from the power board
        // Code will NOT? block until message is received
        // On board computer needs to send the GNSS messages more frequently so that this task does not block for long
        // (Can look into option of using async hal/interrupts but not sure if supported)
        Timer::after(Duration::from_millis(50)).await;
    }
}

// ADC Measurement Task
#[embassy_executor::task]
async fn run_battery_measurement_adc(
    mut adc_1: ADC<'static, ADC1>,
    mut battery_measurement_pin: AdcPin<Gpio3<Analog>, ADC1>,
) {
    loop {
        // Take an ADC Reading

        // ADC is 12 bit resolution (2^12 = 4096) with attenuation 11 db (from 0 to 2.6V)
        // ADC will allow values from 0 to 4096 for voltages between 0 and 3.3V.
        //
        // ADC docs for attenuation: https://docs.espressif.com/projects/esp-idf/en/v4.3/esp32c3/api-reference/peripherals/adc.html#_CPPv415ADC_ATTEN_DB_11
        //
        // Using 11 db we can measure from 0 to 2600 mV (or 2.6V) and we'll take into account the Voltage divider
        // and a battery with voltage specs of:
        // - 4.2V - 100% charge
        // - 3.7V (nominal)
        // - 3.0V cut-off
        // We will use 3.3V for 0% because of the board's buck converter which is lowering the voltage only.
        //
        // Precision factor ( Vref / ADC resolution 2^12): f(p) = 3.3V / 4096
        // R1 - resistor connected on the positive side (+) of the battery
        // R2 - resistor connected on GND (-) of the battery
        // scale = R2 / (R1 + R2) =~ 0.5
        // Formula for calculating the voltage: ADC reading * Precision factor / scale
        //                                    = ADC reading * 3.3 / 4096.0 / 0.5
        //
        // Formula Percentage: (voltage - 3.3) / (4.2 - 3.3) * 100
        // We use 3.3V as the lower

        // let scale = todo!();
        let reading_result: Result<u16, _> = nb::block!(adc_1.read(&mut battery_measurement_pin));
        match reading_result {
            Ok(reading) => {
                let precision = 3.3 / 4096.0;
                let scale = 0.5;
                let voltage = reading as f32 * precision / scale;

                let percentage = (voltage - 3.3) / (4.2 - 3.3) * 100.0;

                println!("Battery (V = {voltage}) {percentage} %");
            }
            Err(_) => {
                println!("Failed to read ADC 1 value")
            }
        };

        Timer::after(Duration::from_secs(5)).await;
    }
}

// LED Blinking Task
#[embassy_executor::task]
async fn run_blinky(mut led: OnboardLed) {
    // LED Blinking Code goes here

    // Make an infinite loop
    loop {
        // Turn on the LED
        led.set_high().unwrap();
        // Delay 500 ms
        Timer::after(Duration::from_millis(500)).await;
        // Turn off the LED
        led.set_low().unwrap();
        // Delay 500 ms
        Timer::after(Duration::from_millis(500)).await;
    }
}
