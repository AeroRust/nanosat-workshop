#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "defmt")]
use defmt_rtt as _;

#[cfg(feature = "cortex-m")]
use cortex_m_rt::{exception, ExceptionFrame};
#[cfg(feature = "cortex-m")]
use panic_probe as _;

// #[cfg(feature = "rp2040")]
#[cortex_m_rt::entry]
fn main() -> ! {
    embassy_rp::pac::SIO.spinlock(31).write_value(1);

    let application = pico_w::Application::init();
    application.run()
}

#[cfg(feature = "rp2040")]
#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    use defmt::error;

    #[cfg(feature = "defmt")]
    error!("HardFault: {:#?}", defmt::Debug2Format(ef));

    // #[cfg(not(feature = "defmt"))]
    // error!("HardFault: {:#?}", ef);

    loop {}
}