#[macro_export]
macro_rules! singleton {
    ($val:expr) => {{
        type T = impl Sized;
        static STATIC_CELL: static_cell::StaticCell<T> = static_cell::StaticCell::new();
        let (x,) = STATIC_CELL.init(($val,));
        x
    }};
}

// /// &'static using `once_cell::sync::OnceCell`
// #[macro_export]
// macro_rules! singleton_ref {
//     ($val:expr) => {{
//         type T = impl Sized + 'static;
//         static ONCE_CELL: once_cell::sync::OnceCell<T> = once_cell::sync::OnceCell::new();
//         let (x,) = ONCE_CELL.get_or_init(|| ($val,));
//         x
//     }};
// }