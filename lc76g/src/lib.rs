#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![feature(type_alias_impl_trait)]

use core::fmt::Write as _;

pub type NmeaSentence = heapless::String<156>;

pub enum GnssMessage {
    /// Sets the baudrate at 115200
    SetBaudrate,
    /// enables all available, to the chip, GNSS providers
    EnableGnssProviders,
}

impl GnssMessage {
    pub fn to_nmea_sentence(&self) -> NmeaSentence {
        let mut string = NmeaSentence::new();

        match self {
            Self::SetBaudrate => {
                // UART
                let port_type = 0;
                // UART 0
                let port_index = 0;
                let baudrate = 115200;
                string
                    .write_fmt(format_args!("$PAIR864,{port_type},{port_index},{baudrate}"))
                    .unwrap();
            }
            Self::EnableGnssProviders => {
                //Search for GPS + GLONASS + Galileo + BDS + QZSS satellites:
                // "$PAIR066,1,1,1,1,1,0*checksum\r\n"
                // Last value is <Reserved> Numeric - Always "0"!
                //
                // Returns a $PAIR001 message.
                // <OutputRate> Numeric -
                // Output rate setting.
                // 0 = Disabled or not supported
                // N = Output once every N position fix(es)
                // Range of N: 1–20. Default value: 1.

                let enable_gps = 1;
                let enable_galileo = 1;
                let enable_glonass = 1;
                let enable_bds = 1;
                let enable_qzss = 1;

                string
                    .write_fmt(format_args!(
                        "$PAIR066,{enable_gps},{enable_glonass},{enable_galileo},{enable_bds},{enable_qzss},0"
                    ))
                    .unwrap();

                // $PAIR066,<GPS_Enabled>,<GLONASS_Enabled>,<Galileo_Enabled>,<BDS_Enabled>,<QZSS_Enabled>,0*<Checksum><CR><LF>
                // Parameter:
                // Result:
                // Returns a $PAIR001 message.
                // <OutputRate> Numeric -
                // Output rate setting.
                // 0 = Disabled or not supported
                // N = Output once every N position fix(es)
                // Range of N: 1–20. Default value: 1.
                // Field Format Unit Description
                // <GPS_Enabled> Numeric - 0 = Disable (DO NOT search for GPS satellites)
                // 1 = Search for GPS satellites
                // <GLONASS_Enabled> Numeric - 0 = Disable (DO NOT search for GLONASS satellites)
                // 1 = Search for GLONASS satellites
                // <Galileo_Enabled> Numeric - 0 = Disable (DO NOT search for Galileo satellites)
                // 1 = Search for Galileo satellites
                // <BDS_Enabled> Numeric - 0 = Disable (DO NOT search for BDS satellites)
                // 1 = Search for BDS satellites
                // <QZSS_Enabled> Numeric - 0 = Disable (DO NOT search for QZSS satellites)
                // 1 = Search for QZSS satellites
            }
        }

        // skip $
        // > The checksum field follows the checksum delimiter character *.
        // > The checksum is the 8-bit exclusive OR of all characters in the sentence, including the
        // > comma (,) delimiter, between but not including the $ and the * delimiters.

        let checksum = string
            .as_bytes()
            .iter()
            .skip(1)
            .fold(0_u8, |char_1, char_2| &char_1 ^ char_2);

        // *<Checksum><CR><LF>
        string.write_fmt(format_args!("*{checksum}\r\n")).unwrap();

        string
    }
}