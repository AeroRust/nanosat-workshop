#![cfg_attr(not(any(feature = "std", test)), no_std)]

#[cfg(any(feature = "rp2040", feature = "rp23"))]
#[doc(inline)]
pub use application::Application;

#[cfg(any(feature = "rp2040", feature = "rp23"))]
pub mod application;
