#![cfg_attr(not(any(features = "std", test)), no_std)]

#![no_main]
#![feature(type_alias_impl_trait)]

// use embassy_sync::{mutex::Mutex, blocking_mutex::raw::CriticalSectionRawMutex};
// use once_cell::sync::OnceCell;

// pub static I2C: OnceCell<Mutex<CriticalSectionRawMutex, I2C<'static, I2C0>>> = OnceCell::new();

// use hal::gpio::{Gpio8, Gpio0, Gpio1, Output, PushPull};

// pub type I2cSda = Gpio0;
// pub type I2cSlc = Gpio1;

pub mod battery;
