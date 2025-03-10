#![cfg_attr(not(any(feature = "std", test)), no_std)]

#[cfg(feature = "rp2040")]
#[doc(inline)]
pub use application::Application;

#[cfg(feature = "rp2040")]
pub mod application;
