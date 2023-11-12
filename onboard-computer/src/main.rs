#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use esp_backtrace as _;

use embassy_executor::Executor;

use hal::peripherals::Peripherals;

use static_cell::make_static;

use onboard_computer::Application;

// static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[hal::entry]
fn main() -> ! {
    esp_println::println!("Init!");
    let peripherals = Peripherals::take();

    esp_println::logger::init_logger_from_env();
    log::info!("Logger is setup");

    let executor = make_static!(Executor::new());

    Application::init(peripherals).run(executor)
}
