#![cfg_attr(not(any(feature = "std", test)), no_std)]

#[doc(inline)]
pub use application::Application;

pub mod application;
