#![cfg_attr(not(any(feature = "std", test)), no_std)]
// #![feature(type_alias_impl_trait)]
// #![feature(impl_trait_in_assoc_type)]

#[cfg(feature = "rp2040")]
#[doc(inline)]
pub use application::Application;

#[cfg(feature = "rp2040")]
pub mod application;
