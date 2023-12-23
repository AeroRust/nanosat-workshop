#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![feature(type_alias_impl_trait)]

#[doc(inline)]
pub use application::Application;

pub mod application;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
