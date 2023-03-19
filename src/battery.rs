// pub type I2C: I2C
use core::result::Result;

use hal::{
    i2c::{Error as I2CError, I2C},
    peripherals::I2C0,
};

pub use max170xx::{Error, Max17043 as MAX170xx};

pub struct Battery {
    // sensor: MAX170xx<&'static Mutex<CriticalSectionRawMutex, I2C<'static, I2C0>>>,
    sensor: MAX170xx<I2C<'static, I2C0>>,
}

impl Battery {
    pub fn new(i2c: I2C<'static, I2C0>) -> Self {
        let sensor = MAX170xx::new(i2c);

        Battery { sensor }
    }

    pub async fn voltage(&mut self) -> Result<f32, Error<I2CError>> {
        self.sensor.voltage()
    }

    pub async fn percentage(&mut self) -> Result<f32, Error<I2CError>> {
        self.sensor.soc()
    }
}
