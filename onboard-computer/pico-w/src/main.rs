#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

#[cfg(feature = "defmt")]
use defmt_rtt as _;

#[cfg(feature = "cortex-m")]
use cortex_m_rt::{exception, ExceptionFrame};
#[cfg(feature = "cortex-m")]
use panic_probe as _;

#[cfg_attr(feature = "rp23", link_section = ".start_block")]
#[cfg_attr(feature = "rp23", used)]
pub static IMAGE_DEF: embassy_rp::block::ImageDef = embassy_rp::block::ImageDef::secure_exe();

// Program metadata for `picotool info`.
// This isn't needed, but it's recomended to have these minimal entries.
#[cfg_attr(feature = "rp23", link_section = ".bi_entries")]
#[cfg_attr(feature = "rp23", used)]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Blinky Example"),
    embassy_rp::binary_info::rp_program_description!(
        c"This example tests the RP Pico on board LED, connected to gpio 25"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

#[cfg(any(feature = "rp2040", feature = "rp23"))]
#[cortex_m_rt::entry]
fn main() -> ! {
    let application = pico_w::Application::init();
    application.run()
}

#[cfg(any(feature = "rp2040", feature = "rp23"))]
#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    use defmt::error;

    #[cfg(feature = "defmt")]
    error!("HardFault: {:#?}", defmt::Debug2Format(ef));

    // #[cfg(not(feature = "defmt"))]
    // error!("HardFault: {:#?}", ef);

    loop {}
}
