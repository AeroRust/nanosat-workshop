//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

fn main() {
    // Load the Wifi network SSID and password
    // https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-env
    {
        dotenv::dotenv().ok();

        let wifi_network_env = "WIFI_NETWORK";
        let wifi_password_env = "WIFI_PASSWORD";
        println!(
            "cargo:rustc-env={wifi_network_env}={}",
            dotenv::var(wifi_network_env).unwrap()
        );
        println!(
            "cargo:rustc-env={wifi_password_env}={}",
            dotenv::var(wifi_password_env).unwrap()
        );
    }

    #[cfg(feature = "rp2040")]
    {
        use std::{env, fs::File, io::Write, path::PathBuf};

        // Put `memory.x` in our output directory and ensure it's
        // on the linker search path.
        let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
        File::create(out.join("memory.x"))
            .unwrap()
            .write_all(include_bytes!("memory.x"))
            .unwrap();
        println!("cargo:rustc-link-search={}", out.display());

        // By default, Cargo will re-run a build script whenever
        // any file in the project changes. By specifying `memory.x`
        // here, we ensure the build script is only re-run when
        // `memory.x` is changed.
        println!("cargo:rerun-if-changed=memory.x");

        println!("cargo:rustc-link-arg-bins=--nmagic");
        println!("cargo:rustc-link-arg-bins=-Tlink.x");
        println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
        println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
