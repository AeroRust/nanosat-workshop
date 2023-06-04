use embassy_executor::Executor;
use embassy_time::{Duration, Timer};
use esp_println::println;
use hal::{
    clock::ClockControl,
    embassy,
    gpio::{Gpio7, Output, PushPull},
    peripherals::{Peripherals, UART0},
    prelude::*,
    system::SystemParts,
    timer::TimerGroup,
    uart::{
        config::{Config, DataBits, Parity, StopBits},
        TxRxPins,
    },
    Rng, Rtc, Uart, IO,
};

use nmea::ParseResult;

/// The Rust ESP32-C3 board has onboard LED on GPIO 7
// pub type OnboardLed = Gpio7<Output<PushPull>>;

static MOCK_SENTENCES: &'static str = include_str!("../../tests/nmea.log");

// #[derive(Default)]
pub struct Application {
    uart0: Uart<'static, UART0>,
    _rng: Rng<'static>,
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

        // Optional code for onboard LED
        // Rust ESP32-C3 schematics: https://raw.githubusercontent.com/esp-rs/esp-rust-board/master/assets/rust_board_v1_pin-layout.png
        // Set GPIO7 as an output, and set its state high initially.
        // let onboard_led = io.pins.gpio7.into_push_pull_output();

        // Setup Random Generator for GNSS Reading

        // Configure UART

        // The Rust ESP32-C3 board has debug UART on pins
        // 20 (TX) and 21 (RX)

        Self { uart0, _rng }
    }

    pub fn run(self, executor: &'static mut Executor) -> ! {
        executor.run(|spawner| {
            spawner.must_spawn(uart_comm(self.uart0));
            spawner.must_spawn(gnss(self.rng));
        })
    }
}

#[embassy_executor::task]
async fn gnss(mut rng: Rng<'static>) {
    // This task parses NMEA sentences simulated from a GNSS data log file
    // The task picks random sentences from a log file and looks out for GNS and GSV messages
    // The task prints the number of sats in GNS data and number of sats in view from GSV data
}

#[embassy_executor::task]
async fn uart_comm(mut uart: Uart<'static, UART0>) {
    // This Task Reads Battery Percentage Value sent from Power System every 1 second
}
