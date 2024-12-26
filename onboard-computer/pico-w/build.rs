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
        println!("cargo::rerun-if-changed=../.env");

        dotenv::dotenv().ok();

        for (env_var, env_value) in dotenv::vars() {
            println!("cargo:rustc-env={env_var}={env_value}");
            println!("cargo:rerun-if-env-changed={env_var}");
        }
    }

    use std::{env, fs::File, io::Write, path::PathBuf};

    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    #[cfg(any(feature = "rp2040", feature = "rp23"))]
    {
        #[cfg(feature = "rp2040")]
        let memory_x_content = include_bytes!("memory.x");
        #[cfg(feature = "rp23")]
        let memory_x_content = include_bytes!("memory_rp23.x");

        File::create(out.join("memory.x"))
            .unwrap()
            .write_all(memory_x_content)
            .unwrap();
        println!("cargo:rustc-link-search={}", out.display());

        // By default, Cargo will re-run a build script whenever
        // any file in the project changes. By specifying `memory.x`
        // here, we ensure the build script is only re-run when
        // `memory.x` is changed.
        #[cfg(feature = "rp2040")]
        println!("cargo:rerun-if-changed=memory.x");
        #[cfg(feature = "rp23")]
        println!("cargo:rerun-if-changed=memory_rp23.x");
    }

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    // we do not need it for RP23xx
    #[cfg(feature = "rp2040")]
    println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
    #[cfg(feature = "defmt")]
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");

    println!("cargo:rerun-if-changed=build.rs");
}
