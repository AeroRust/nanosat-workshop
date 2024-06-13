#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_assoc_type)]

pub use application::Application;

pub mod application;
