#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![feature(type_alias_impl_trait)]

#[doc(inline)]
pub use application::Application;

pub mod application;
