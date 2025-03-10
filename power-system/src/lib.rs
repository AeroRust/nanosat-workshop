#![cfg_attr(not(any(feature = "std", test)), no_std)]

pub use application::Application;

pub mod application;
