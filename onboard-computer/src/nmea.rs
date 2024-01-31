use core::{iter::Cycle, str::Lines};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};

use embedded_hal::prelude::_embedded_hal_blocking_rng_Read;

use hal::Rng;

pub static MOCK_SENTENCES: &'static str = include_str!("../../tests/nmea.log");

pub struct NmeaReceiver {
    mock_sentences: Mutex<CriticalSectionRawMutex, Cycle<Lines<'static>>>,
    rng: Mutex<CriticalSectionRawMutex, Rng>,
}

impl NmeaReceiver {
    pub fn new(rng: Mutex<CriticalSectionRawMutex, Rng>) -> Self {
        let sentences_iterator = MOCK_SENTENCES.lines().cycle();

        Self {
            mock_sentences: Mutex::new(sentences_iterator),
            rng,
        }
    }

    /// We can parse the `&str` and look for the following sentences which have
    /// longitude and latitude information:
    /// - GGA
    /// - RMC
    pub async fn receive(&self) -> &str {
        let milliseconds = {
            let mut rng = self.rng.lock().await;
            let mut random_bytes = [0_u8];
            rng.read(&mut random_bytes).expect("Should get random byte");

            255 - random_bytes[0]
        };
        
        Timer::after(Duration::from_millis(milliseconds.into())).await;
        let mut sentence_guard = self.mock_sentences.lock().await;


        sentence_guard.next().expect("Should always have a next sentence since we have a Cycle iterator")
    }
}
