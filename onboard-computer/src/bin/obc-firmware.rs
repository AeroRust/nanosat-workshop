#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "esp32-c3")]
use esp_backtrace as _;

#[cfg(feature = "defmt")]
use defmt_rtt as _;

#[cfg(feature = "cortex-m")]
use cortex_m_rt::{exception, ExceptionFrame};
#[cfg(feature = "cortex-m")]
use panic_probe as _;


#[cfg(feature = "esp32-c3")]
use hal::peripherals::Peripherals;

use static_cell::make_static;

#[cfg(feature = "esp32-c3")]
use esp32_c3::Application;

// static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[cfg(feature = "esp32-c3")]
#[hal::entry]
fn main() -> ! {
    esp_println::println!("Init!");
    let peripherals = Peripherals::take();

    esp_println::logger::init_logger_from_env();
    log::info!("Logger is setup");

    let executor = make_static!(Executor::new());

    Application::init(peripherals).run(executor)
}

#[cfg(feature = "cortex-m")]
#[cortex_m_rt::entry]
fn main() -> ! {
    let application = pico_w::Application::init();
    application.run()
}

#[cfg(feature = "cortex-m")]
#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    use defmt::error;

    #[cfg(feature = "defmt")]
    error!("HardFault: {:#?}", defmt::Debug2Format(ef));

    // #[cfg(not(feature = "defmt"))]
    // error!("HardFault: {:#?}", ef);

    loop {}
}

#[cfg(feature = "std")]
fn main() {}
