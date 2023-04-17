#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

// use {defmt_rtt as _, panic_probe as _};
// use defmt_rtt as _;

// use defmt::{debug, error};
use esp_backtrace as _;

use embassy_executor::Executor;

use hal::peripherals::Peripherals;

use static_cell::StaticCell;

use nanosat::Application;

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[hal::entry]
fn main() -> ! {
    esp_println::println!("Init!");
    let peripherals = Peripherals::take();

    let executor = EXECUTOR.init(Executor::new());

    Application::init(peripherals).run(executor)
}
