//! Assigns relocatable loader identities to the C ABI shared artifact.

use std::env;

fn main() {
    match env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => {
            println!(
                "cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libfontdone_c_abi.dylib"
            );
        }
        Ok("linux") => {
            println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libfontdone_c_abi.so");
        }
        _ => {}
    }
}
