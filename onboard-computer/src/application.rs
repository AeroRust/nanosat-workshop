use embassy_executor::Executor;
use embassy_time::{Duration, Timer};
use esp_println::println;
use hal::{
    clock::ClockControl,
    embassy,
    gpio::{Gpio7, Output, PushPull},
    interrupt,
    peripherals::{Interrupt, Peripherals, UART0, UART1},
    prelude::*,
    system::SystemParts,
    timer::TimerGroup,
    uart::{
        config::{Config, DataBits, Parity, StopBits},
        TxRxPins,
    },
    Priority, Rng, Rtc, Uart, IO,
};

use nmea::ParseResult;

/// The Rust ESP32-C3 board has onboard LED on GPIO 7
pub type OnboardLed = Gpio7<Output<PushPull>>;

static MOCK_SENTENCES: &'static str = include_str!("../../tests/nmea.log");

pub struct Application {
    // TODO: Uncomment when you create a `Uart` instance of the `UART1` peripheral
    uart: Uart<'static, UART1>,
    // TODO: Uncomment when you create a `Rng` instance
    rng: Rng<'static>,
    // TODO: Uncomment when you create the `OnboardLed` instance
    onboard_led: OnboardLed,
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

        // Setup peripherals for application
        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

        // Onboard LED
        // Rust ESP32-C3 schematics: https://raw.githubusercontent.com/esp-rs/esp-rust-board/master/assets/rust_board_v1_pin-layout.png
        // Set GPIO7 as an output, and set its state high initially.
        let onboard_led = io.pins.gpio7.into_push_pull_output();

        // Setup Random Generator for GNSS Reading
        // Hal example: https://github.com/esp-rs/esp-hal/blob/main/esp32c3-hal/examples/rng.rs
        let rng = Rng::new(peripherals.RNG);
        // let rng = todo!("Initialize the Random generator");

        // The Rust ESP32-C3 board has debug UART on pins 21 (TX) and 20 (RX)
        // but you should use GPIO pins 0 (TX) and 1 (RX) for sending and receiving data respectively
        // to/from the `power-system` board.
        // TODO: Configure the UART 1 peripheral
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
        interrupt::enable(Interrupt::UART1, Priority::Priority1).unwrap();

        Self {
            uart: uart1,
            rng,
            onboard_led,
        }
    }

    /// Runs the application by spawning each of the [`Application`]'s tasks
    pub fn run(self, executor: &'static mut Executor) -> ! {
        executor.run(|spawner| {
            spawner.must_spawn(run_uart(self.uart));
            spawner.must_spawn(run_gnss(self.rng));
        })
    }
}

/// # Exercise: Parse GNSS data from NMEA 0183 sentences
///
/// This task parses NMEA sentences simulated from a GNSS data log file
/// The task picks random sentences from a log file and looks out for `GNS` and `GSV` messages
///
///
/// Print the number of satellites from the GNS sentence and the satellites in view from the GSV sentence
///
/// `nmea` crate docs: <https://docs.rs/nmea>
///
/// 0. Add the `nmea` crate to the `Cargo.toml` of the `onboard-computer`
/// - You should exclude the `default-features` of the crate, as we operate in `no_std` environment
/// - Alternate solution: You can add the crate to the `workspace` dependencies of the project in the `Cargo.toml` of the project
///   For more details see: <https://doc.rust-lang.org/cargo/reference/workspaces.html#the-dependencies-table>
///
/// 1. Use the `nmea` crate to parse the sentences
/// 2. Print the parsing result (for debugging purposes) using `esp_println::println!()`
/// 3. Use a match on the result and handle the cases:
/// - GNS - print "Number of satellites: {x}" field
/// - GSV - print "Satellites in View: {x}" field
#[embassy_executor::task]
async fn run_gnss(mut rng: Rng<'static>) {
    loop {
        let num = rng.random() as u8;
        let sentence = MOCK_SENTENCES.lines().nth(num as usize).unwrap();

        // 1. Use the `nmea` crate to parse the sentences
        // TODO: Uncomment line and finish the `todo!()`
        // let parse_result = todo!("call nmea::parse_str");
        let parse_result = nmea::parse_str(sentence);
        // 2. Print the parsing result (for debugging purposes) using `esp_println::println!()`
        println!("{:?}", parse_result);
        // 3. Use a match on the result and handle the sentences:
        // - GNS
        // - GSV
        match parse_result {
            Ok(ParseResult::GNS(gns_data)) => {
                println!("GNS: Number of satellites: {}", gns_data.nsattelites);
            }
            Ok(ParseResult::GSV(gsv_data)) => {
                println!("GSV: Satellites in view: {}", gsv_data._sats_in_view);
            }
            _ => {}
        }
        Timer::after(Duration::from_secs(2)).await;
    }
}

/// # Exercise: Send Battery percentage over UART
///
/// 1. create an infinite loop that
/// 2. tries to read the Battery Percentage value sent from the Power System,
/// 3. prints the value on success or the error on failure (using Debug formatting),
/// 4. Repeat the read every 20 milliseconds
#[embassy_executor::task]
async fn run_uart(mut uart: Uart<'static, UART1>) {
    loop {
        match nb::block!(uart.read()) {
            Ok(battery_percentage) => {
                println!("Battery: {battery_percentage} %");
            }
            Err(err) => {
                println!("Error: {err:?}")
            }
        }
        Timer::after(Duration::from_millis(20)).await;
    }
}
