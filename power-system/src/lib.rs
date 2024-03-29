#![cfg_attr(not(any(feature = "std", test)), no_std)]

#![no_main]
#![feature(type_alias_impl_trait)]

pub use application::Application;

pub mod application;
pub mod battery;
pub mod helper;
