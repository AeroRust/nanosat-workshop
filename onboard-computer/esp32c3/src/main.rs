#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

use esp_backtrace as _;

#[cfg(feature = "defmt")]
use defmt_rtt as _;

use esp_hal::peripherals::Peripherals;

use esp32c3::Application;

// static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[esp_hal::entry]
fn main() -> ! {
    // esp_println::println!("Init!");
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        // Configure the CPU to run at the maximum frequency.
        config.cpu_clock = esp_hal::prelude::CpuClock::max();
        config
    });

    // esp_println::logger::init_logger_from_env();
    // log::info!("Logger is setup");

    // let executor = static_cell::make_static!(embassy_executor::Executor::new());
// 
    Application::init(peripherals).run()
}