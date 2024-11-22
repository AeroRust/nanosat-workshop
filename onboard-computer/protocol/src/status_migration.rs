


#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SendMessage2 {
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
    /// Device status
    #[cfg(feature = "status")]
    Status(Status),
    #[cfg(feature = "status")]
    NewStatus(NewStatus),
}

#[cfg(feature = "postcard")]
impl SendMessage2 {
    pub fn to_radio<'a, 'b>(&'a self, buf: &'b mut [u8]) -> Result<&'b mut [u8], postcard::Error> {
        postcard::to_slice(self, buf)
    }
    pub fn from_radio(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}


#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
pub struct Status {
    pub internal_temperature: f32,
    pub battery_percentage: u8,
}