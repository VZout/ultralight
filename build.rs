use std::env;
use std::path::PathBuf;

fn main() {
    println!(
        "cargo:rustc-link-search={}/libs/",
        env!("CARGO_MANIFEST_DIR")
    );

    println!("cargo:rustc-link-lib=Ultralight");
    println!("cargo:rustc-link-lib=UltralightCore");
    println!("cargo:rustc-link-lib=WebCore");
    println!("cargo:rustc-link-lib=AppCore");

    let api_path = format!("{}/api/", env!("CARGO_MANIFEST_DIR"));

    let bindings = bindgen::Builder::default()
        .header(format!("{}{}", api_path, "AppCore/CAPI.h"))
        .clang_arg(format!("-I{}", api_path))
        .clang_arg("-Duintptr_t=unsigned __int64")
        .clang_arg("-Dintptr_t=__int64")
        .derive_default(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("app_core.rs"))
        .expect("Couldn't write bindings!");
}
