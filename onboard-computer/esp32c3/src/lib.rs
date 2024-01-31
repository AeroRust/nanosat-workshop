#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![feature(type_alias_impl_trait)]

#[doc(inline)]
use application::Application;

pub mod application;