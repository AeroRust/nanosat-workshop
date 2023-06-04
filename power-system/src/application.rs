use embassy_executor::Executor;

use embassy_time::{Duration, Timer};

use esp_println::println;

use hal::gpio::Analog;

use hal::{
    adc::{AdcConfig, AdcPin, Attenuation, ADC, ADC1},
    clock::ClockControl,
    embassy,
    gpio::{Gpio3, Gpio8, Output, PushPull},
    peripherals::{Peripherals, UART0},
    prelude::*,
    system::SystemParts,
    timer::TimerGroup,
    uart::{
        config::{Config, DataBits, Parity, StopBits},
        TxRxPins,
    },
    Rtc, Uart, IO,
};

use core::sync::atomic::{AtomicU8, Ordering};

// #[derive(Default)]
pub struct Application {
    _adc: ADC<'static, ADC1>,
    _uart0: Uart<'static, UART0>,
    // _onboard_led: OnboardLed,
    _battery_measurement_pin: AdcPin<Gpio3<Analog>, ADC1>,
}

impl Application {
    pub fn init(peripherals: Peripherals) -> Self {
        let system: SystemParts = peripherals.SYSTEM.split();
        let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

        let mut rtc = Rtc::new(peripherals.RTC_CNTL);
        let mut peripheral_clock_control = system.peripheral_clock_control;
        let timer_group0 =
            TimerGroup::new(peripherals.TIMG0, &clocks, &mut peripheral_clock_control);
        let mut wdt0 = timer_group0.wdt;
        let mut timer_group1 =
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

        // Setup peripherals for application

        // Olimex ESP32-C3 schematics: https://raw.githubusercontent.com/OLIMEX/ESP32-C3-DevKit-Lipo/main/HARDWARE/ESP32-C3-DevKit-Lipo_Rev_B/ESP32-C3-DevKit-Lipo_Rev_B.pdf
        // Configure GPIO and set GPIO8 (LED pin) as an output

        // Configure ADC Here

        // Configure UART Here

        Self {
            _adc,
            // _onboard_led,
            _battery_measurement_pin,
            _uart0,
        }
    }

    pub fn run(self, executor: &'static mut Executor) -> ! {
        // executor.run(|spawner| {
        //     spawner.must_spawn(battery_measurement_adc(
        //         self.adc,
        //         self.battery_measurement_pin,
        //     ));
            // spawner.must_spawn(blink(self.onboard_led));
            // spawner.must_spawn(uart_comm(self.uart0));
        })
    }
}

// UART Transmit Communication Task
#[embassy_executor::task]
async fn uart_comm(mut uart: Uart<'static, UART0>) {
    // This communication task will be executed every 1 second
    // First the battery percentage will be sent to the power system
    // Second, the task will enter blocking state until new GNSS message is recieved from power system
    loop {
        // Transmit Operations
        // First we send the battery percentage value to the Power system

        // Recieve Operations
        // Second we poll UART reciever to check if any messages are recieved
        // This code will recieve NEMA messages from the power board
        // Code will NOT? block until message is recieved
        // On board computer needs to send the GNSS messages more frequently so that this task does not block for long
        // (Can look into option of using async hal/interrupts but not sure if supported)
    }
}

// ADC Measurement Task
#[embassy_executor::task]
async fn battery_measurement_adc(
    mut adc_1: ADC<'static, ADC1>,
    mut battery_measurement_pin: AdcPin<Gpio3<Analog>, ADC1>,
) {
    loop {
        // Take an ADC Reading

        // ADC is 12 bit resolution
        // Resolution = Vref/Full Scale = 3.3V/2^12
        // Measured Voltage =  Resolution * ADC Reading = reading * 3.3V/2^12
        // Battery Percentage = (Measured Voltage/Full Battery Voltage) * 100% = (3.7 - (reading * 3.3V / 2^12))/(3.7-3.3))/100
    }
}

// LED Blinking Task
#[embassy_executor::task]
async fn blink(mut led: OnboardLed) {
    // LED Blinking Code goes here
}
