#![cfg_attr(not(any(feature = "std", test)), no_std)]

#[cfg(feature = "postcard")]
use postcard::experimental::schema::Schema;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SendPacket {
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
        /// Gets a quaternion (mint::Quaternion<f32>) reading from the BNO055. Must be in a sensor fusion (IMU) operating mode.
        #[cfg_attr(feature = "defmt-03", defmt(Debug2Format))]
        quaternion: bno055::mint::Quaternion<f32>,
    },
    #[cfg(feature = "GNSS")]
    GnssData(GnssData),
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
