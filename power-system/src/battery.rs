// pub type I2C: I2C
use core::{ops, result::Result};

// #[cfg(feature = "defmt")]
// use defmt::Format;

use defmt::Format;
use embassy_time::{Duration, Timer};
use esp_println::println;
use hal::{
    adc::{AdcPin, ADC, ADC1},
    gpio::{Analog, Gpio3, Gpio4, Input},
    prelude::*,
};

use heapless::{HistoryBuffer, Vec};

use crate::helper::find_mediana;

pub type BatteryMeasurementPin = AdcPin<Gpio3<Analog>, ADC1>;
pub type PowerSensePin = AdcPin<Gpio4<Analog>, ADC1>;

/// A voltage divider on GPIO3 with 470 Omhs each and safe to read with a scale factor.
///
// #[derive(Debug)]
// #[cfg_attr(feature = "defmt", derive(Format))]
pub struct BatteryMeasurement<const R1: usize, const R2: usize> {
    pub analog_pin: BatteryMeasurementPin,
    // The medianas of the last X battery measurements as an ADC value
    // it's easier to compare and sort
    /// Buffer should always be with an odd size
    pub last_measurements: HistoryBuffer<u16, 101>,
    pub voltage_divider: VoltageDivider<R1, R2>,
}

pub struct Battery {
    pub cut_out_voltage: f32,
    pub charged_voltage: f32,
}

#[derive(Debug)]
pub struct Error;

/// The number of measurements that we do before averaging the ADC value
pub const MEASUREMENTS: usize = 15;

impl<const R1: usize, const R2: usize> BatteryMeasurement<R1, R2> {
    /// for internal 3.3v reference, add correct scale factor once ADC calibration is implemented:
    /// <https://github.com/esp-rs/esp-hal/issues/326#issuecomment-1438911773>
    // pub const PRECISION_FACTOR: f32 = 1.315772567 * 3.3 / 4096.0;
    pub const PRECISION_FACTOR: f32 = 3.3 / 4096.0;

    pub async fn measure_once<'a>(&mut self, adc: &mut ADC<'a, ADC1>) -> Result<u16, Error> {
        self.analog_pin.pin.internal_pull_down(false);
        // FIXME: Is there an async reading at all in esp32c3_hal?!
        let max_tries = 30;
        for try_index in 0..max_tries {
            match adc.read(&mut self.analog_pin) {
                Ok(value) => return Ok(value),
                Err(nb::Error::WouldBlock) => {}
                Err(err) => println!(
                    "{}/{} tries ADC read error: {:?}",
                    try_index, max_tries, err
                ),
            }
            Timer::after(Duration::from_micros(100)).await;
        }

        Err(Error)
    }

    pub async fn measure_percentage<'a>(
        &mut self,
        adc: &mut ADC<'a, ADC1>,
        battery: &Battery,
    ) -> Result<f32, Error> {
        let mut current_measurements: Vec<u16, MEASUREMENTS> = Vec::new();
        let mut sum = 0.0;

        for i in 1..=MEASUREMENTS {
            // read ADC level
            // let checkpoint = embassy_time::Instant::now();
            let adc_level = self.measure_once(adc).await?;

            // defmt::debug!(
            //     "ADC read took: {} microseconds",
            //     checkpoint.elapsed().as_micros()
            // );

            current_measurements.push(adc_level).expect("Should fit");
            sum += adc_level as f32;

            // skip last timer
            if i != MEASUREMENTS {
                Timer::after(Duration::from_millis(1)).await;
            }
        }

        // calculate the average value
        let average = sum / MEASUREMENTS as f32;
        println!("Battery average ADC value: {}", average);

        // ADC is 12 bit resolution
        // Resolution = 1.1V/2^12 = 1.1/4095 = 0.268555 mV for every 1_u16 of ADC value
        let voltage_avg = average * Self::PRECISION_FACTOR;
        println!("Divider: {}", self.voltage_divider.divider());
        let voltage_avg = voltage_avg / self.voltage_divider.divider();

        // current mediana
        let current_adc_mediana = {
            let mut measurements: Vec<u16, 101> =
                Vec::from_slice(current_measurements.as_slice()).expect("Should never fail");

            find_mediana(&mut measurements)
        };
        println!("Current ADC mediana: {}", current_adc_mediana);
        let voltage_mediana = current_adc_mediana as f32 * Self::PRECISION_FACTOR;
        let voltage_mediana = voltage_mediana / self.voltage_divider.divider();

        let _percentage_avg = self.percentage(voltage_avg, battery);
        let percentage_mediana = self.percentage(voltage_mediana, battery);

        // println!(
        //     "Current average of {} measurements - Battery voltage: {}V ; percentage: {}%",
        //     MEASUREMENTS,
        //     voltage_avg,
        //     percentage_avg
        // );
        println!(
            "Current mediana of {} measurements - Battery voltage: {}V; percentage: {}%",
            MEASUREMENTS, voltage_mediana, percentage_mediana
        );

        // write current_mediana to history
        self.last_measurements.write(current_adc_mediana);

        // println!(
        //     "History mediana of {} measurements - ADC value: {}; percentage: {}%",
        //     MEASUREMENTS, self.historic_mediana(), self.historic_percentage(battery),
        // );

        if percentage_mediana < 100.0 {
            Ok(percentage_mediana)
        } else {
            Ok(100.0)
        }
    }

    pub fn historic_mediana(&self) -> u16 {
        // calculate the mediana of the new history
        let mut history = Vec::<_, 101>::new();
        history
            .extend_from_slice(self.last_measurements.as_slice())
            .expect("Should not fail");

        find_mediana(&mut history)
    }

    pub fn historic_percentage(&self, battery: &Battery) -> f32 {
        let adc_mediana = self.historic_mediana();
        let voltage_mediana = self.adc_to_voltage(adc_mediana);

        self.percentage(voltage_mediana, battery)
    }

    /// adc value to voltage
    pub fn adc_to_voltage(&self, value: u16) -> f32 {
        let measured_voltage = value as f32 * Self::PRECISION_FACTOR;
        let actual_voltage = measured_voltage / self.voltage_divider.divider();

        actual_voltage
    }

    /// For a battery with 4.2V maximum charge and 3.0V cut out voltage:
    ///
    // (voltage - 3.0) / (4.2 - 3.0) * 100
    pub fn percentage(&self, voltage: f32, battery: &Battery) -> f32 {
        // (voltage - 3.0) / (4.2 - 3.0) * 100
        ((voltage - battery.cut_out_voltage) / (battery.charged_voltage - battery.cut_out_voltage))
            * 100.0
    }
}

// #[derive(Debug)]
// #[cfg_attr(feature = "defmt", derive(Format))]
pub struct PowerSense(PowerSensePin);

/// R2 is towards the positive (+) side
/// R1 is towards the GND
///
/// <https://ohmslawcalculator.com/voltage-divider-calculator>
// #[cfg_attr(feature = "defmt", derive(Format))]
#[derive(Debug, Clone, Copy, Format)]
pub struct VoltageDivider<const R1: usize, const R2: usize>;

impl<const R1: usize, const R2: usize> VoltageDivider<R1, R2> {
    /// The voltage divider value
    pub fn divider(&self) -> f32 {
        if R2 == 0 {
            return 1.0;
        }
        let r1_f32 = R1 as f32;
        let r2_f32 = R2 as f32;

        r2_f32 / (r1_f32 + r2_f32)
    }
}

impl<const R1: usize, const R2: usize> ops::Div<VoltageDivider<R1, R2>> for f32 {
    type Output = f32;

    fn div(self, rhs: VoltageDivider<R1, R2>) -> Self::Output {
        self / rhs.divider()
    }
}

/// Markup trait
pub trait ApiType {}

/// Blocking API type (no `async` & `.await`)
pub struct Blocking;
impl ApiType for Blocking {}

/// Asynchronous API type (using `async` & `.await`)
pub struct Async;
impl ApiType for Async {}

// pub struct Voltmeter<API, const R1: usize, const R2: usize> {
//     api: PhantomData<API>,
//     voltage_divider: PhantomData<VoltageDivider<R1, R2>>,
//     // const VOLTAGE_DIVIDER_R1: usize;
//     // const VOLTAGE_DIVIDER_R2: usize;
// }

// impl<const R1: usize, const R2: usize, API> Voltmeter<API, R1, R2>
// where
//     API: ApiType,
// {
//     pub fn new(voltage_divider: VoltageDivider<R1, R2>, api: API) -> Self {
//         Self {
//             voltage_divider,
//             api: PhantomData,
//         }
//     }
// }

// impl<const R1: usize, const R2: usize> Voltmeter<Blocking, R1, R2> {
//     fn measure(&self) {

//     }
// }
