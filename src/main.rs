#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor::Executor;
use embassy_time::{Duration, Timer};

use fugit::HertzU32;
use hal::{
    clock::ClockControl,
    embassy,
    gpio::{Gpio8, Output, PushPull},
    i2c::I2C,
    peripherals::Peripherals,
    prelude::*,
    system::SystemParts,
    timer::TimerGroup,
    Rtc, IO,
};

// use {defmt_rtt as _, panic_probe as _};
// use defmt_rtt as _;

// use defmt::{debug, error};

use esp_backtrace as _;

use nanosat::battery::Battery;

use static_cell::StaticCell;

// pub static I2C_INSTANCE: StaticCell<I2C<I2C0>> = StaticCell::new();
pub static BATTERY: StaticCell<Battery> = StaticCell::new();

static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[hal::entry]
fn main() -> ! {
    esp_println::println!("Init!");
    let peripherals = Peripherals::take();
    let mut system: SystemParts = peripherals.SYSTEM.split();
    let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    let mut rtc = Rtc::new(peripherals.RTC_CNTL);
    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    let mut wdt0 = timer_group0.wdt;
    let timer_group1 = TimerGroup::new(peripherals.TIMG1, &clocks);
    let mut wdt1 = timer_group1.wdt;

    // Disable watchdog timers
    rtc.swd.disable();
    rtc.rwdt.disable();
    wdt0.disable();
    wdt1.disable();

    #[cfg(feature = "embassy-time-systick")]
    embassy::init(
        &clocks,
        hal::systimer::SystemTimer::new(peripherals.SYSTIMER),
    );

    #[cfg(feature = "embassy-time-timg0")]
    embassy::init(&clocks, timer_group0.timer0);

    // Setup peripherals for application

    // Set GPIO5 as an output, and set its state high initially.
    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    let led = io.pins.gpio8.into_push_pull_output();

    // FIXME: We're not going to share I2C for now but we have to make it possible to share it!
    // Set I2C
    // let i2c = I2C_INSTANCE.init_with(move || {
    //     debug!("Initialize I2C static");
    //     // GPIO 0
    //     let sda = io.pins.gpio0;
    //     // GPIO 1
    //     let scl = io.pins.gpio1;

    //     // enable I2C
    //     system.peripheral_clock_control.enable(Peripherals::I2cExt0);

    //     I2C::new(
    //         peripherals.I2C0,
    //         sda,
    //         scl,
    //         clocks.i2c_clock,
    //         Peripherals::I2cExt0,
    //         &clocks
    //     )
    // });

    let i2c = {
        // defmt::debug!("Initialize I2C");
        // GPIO 0
        let sda = io.pins.gpio0;
        // GPIO 1
        let scl = io.pins.gpio1;

        I2C::new(
            peripherals.I2C0,
            sda,
            scl,
            HertzU32::Hz(1_000),
            &mut system.peripheral_clock_control,
            &clocks,
        )
    };

    // set up battery the instance
    let battery = Battery::new(i2c);
    // let battery = BATTERY.init_with(move || {
    //     // defmt::debug!("Initialize Battery static");

    // });

    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(battery_measurement(battery)).ok();
        spawner.spawn(blink(led)).ok();
        spawner.spawn(run()).ok();
    });
}

#[embassy_executor::task]
async fn blink(mut led: Gpio8<Output<PushPull>>) {
    loop {
        led.set_high().expect("Should set High");
        esp_println::println!("async LED ON");
        Timer::after(Duration::from_millis(100)).await;

        led.set_low().expect("Should set Low");
        esp_println::println!("async LED OFF");
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
async fn battery_measurement(mut battery: Battery) {
// async fn battery_measurement(battery: &'static mut Battery) {
    loop {
        match (battery.percentage().await, battery.voltage().await) {
            (Ok(percentage), Ok(voltage)) => {
                esp_println::println!("Percentage: {:.2}, Voltage: {:.2}V", percentage, voltage);
            }
            (Ok(percentage), Err(voltage_err)) => {
                esp_println::println!(
                    "Percentage: {:.2}, Voltage: Err({:?})",
                    percentage,
                    voltage_err
                );
            }

            (Err(percentage_err), Ok(voltage)) => {
                esp_println::println!(
                    "Percentage: Err({:?}), Voltage: {:.2})",
                    percentage_err,
                    voltage
                );
            }
            (Err(percentage_err), Err(voltage_err)) => {
                esp_println::println!(
                    "Percentage: Err({:?}), Voltage: Err({:?})",
                    percentage_err,
                    voltage_err
                );
            }
        }

        Timer::after(Duration::from_secs(3)).await;
    }
}

#[embassy_executor::task]
async fn run() {
    loop {
        esp_println::println!("Bing!");
        Timer::after(Duration::from_millis(5_000)).await;
    }
}
