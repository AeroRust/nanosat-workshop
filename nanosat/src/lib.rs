#![cfg_attr(not(any(feature = "std", test)), no_std)]

pub use macros::*;

mod macros;
pub mod message;