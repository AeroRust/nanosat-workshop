#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

#[cfg(feature = "esp32-c3")]
use esp_backtrace as _;
#[cfg(feature = "esp32-c3")]
use esp_println as _;

#[cfg(any(all(feature = "rp2040", feature = "defmt"), all(feature = "esp32-c3", feature = "defmt")))]
use defmt_rtt as _;

#[cfg(feature = "cortex-m")]
use cortex_m_rt::{exception, ExceptionFrame};
#[cfg(feature = "cortex-m")]
use panic_probe as _;

// static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[cfg(feature = "esp32-c3")]
#[esp_hal::entry]
fn main() -> ! {
    use esp32c3::Application;
    // esp_println::println!("Init!");
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        // Configure the CPU to run at the maximum frequency.
        config.cpu_clock = esp_hal::prelude::CpuClock::max();
        config
    });

    #[cfg(feature = "log")]
    {
        esp_println::logger::init_logger_from_env();
        log::info!("log: Logger is setup");
    }

    #[cfg(feature = "defmt")]
    {
        // esp_println::logger::init_logger_from_env();
        defmt::info!("defmt: Logger is setup");
    }

    Application::init(peripherals).run()
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
