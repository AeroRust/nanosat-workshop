#![cfg_attr(not(any(feature = "std", test)), no_std)]

pub use median::calculate_median;

mod median;

// #[cfg(feature = "postcard")]
// use postcard::experimental::schema::Schema;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SendPacket {
    // TODO: Change to SocketAddrV4
    pub remote: ([u8; 4], u16),
    pub message: SendMessage,
}

#[cfg(feature = "postcard")]
impl SendMessage {
    pub fn to_radio<'a, 'b>(&'a self, buf: &'b mut [u8]) -> Result<&'b mut [u8], postcard::Error> {
        postcard::to_slice(self, buf)
    }
    pub fn from_radio(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SendMessage {
    #[cfg(feature = "BMP388")]
    PressureData {
        sensor_data: bmp388::SensorData,
        altitude: f64,
    },
    #[cfg(feature = "BNO055")]
    ImuData {
        /// Returns current accelerometer data in m/s^2 units. Available only in modes in which accelerometer is enabled.
        #[cfg_attr(feature = "defmt-03", defmt(Debug2Format))]
        accel_data: bno055::mint::Vector3<f32>,
        /// Returns current gyroscope data in deg/s units. Available only in modes in which gyroscope is enabled.
        #[cfg_attr(feature = "defmt-03", defmt(Debug2Format))]
        gyro_data: bno055::mint::Vector3<f32>,
        /// Gets a quaternion ([`bno055::mint::Quaternion<f32>`]) reading from the BNO055.
        /// Must be in a sensor fusion (IMU) operating mode.
        #[cfg_attr(feature = "defmt-03", defmt(Debug2Format))]
        quaternion: bno055::mint::Quaternion<f32>,
    },
    #[cfg(feature = "GNSS")]
    GnssData(GnssData),
    #[cfg(feature = "status")]
    /// Device status
    Status(Status),
}

#[cfg(feature = "status")]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
// #[cfg_attr(feature = "serde", serde(untagged))]
#[derive(Debug)]
pub struct Status {
    pub internal_temperature: f32,
    /// Will be None if we are powered by VBUS (USB)
    pub battery: Option<Battery>,
    /// # Returns
    ///
    /// `Some(true)` or `Some(false)` to indicated that USB feature (`run-usb`) is **enabled**
    /// and the status of the USB connection is Connected (`true`) or Not connected (`false`)
    /// respectively.
    #[cfg_attr(feature = "serde", serde(default))]
    pub usb_connected: Option<bool>,
    /// # Returns
    ///
    /// `Some(true)` or `Some(false)` to indicated that USB feature (`run-usb`) is **enabled**
    /// and the status of the WIFI is Connected (`true`) or Not connected (`false`)
    /// respectively to the WIFI network provided in the firmware.
    #[cfg_attr(feature = "serde", serde(default))]
    pub radio_connected: Option<bool>,
}

#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Battery {
    /// Single measured median voltage at provided ADC pin for XXXXX times between XXXX ms each.
    ///
    /// Should be between 0V & 3V3 for the Pico!
    pub voltage: f32,
    /// 
    pub percentage: u8,
}

#[cfg(feature = "GNSS")]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
// #[cfg_attr(feature = "serde", serde(untagged))]
#[derive(Debug)]
pub enum GnssData {
    GSA(nmea::sentences::GsaData),
    GSV(nmea::sentences::GsvData),
    RMC(nmea::sentences::RmcData),
    GLL(nmea::sentences::GllData),
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ReceiveMessage {}

#[cfg(feature = "postcard")]
impl ReceiveMessage {
    pub fn to_radio<'a, 'b>(&'a self, buf: &'b mut [u8]) -> Result<&'b mut [u8], postcard::Error> {
        postcard::to_slice(self, buf)
    }
    pub fn from_radio(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}
