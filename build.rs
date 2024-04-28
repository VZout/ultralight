// TODO: Can potentially remove `AppCore` linking.

fn main() {
    #[cfg(feature = "requires_dll")]
    {
        println!(
            "cargo:rustc-link-search={}/ultralight-lib/",
            env!("CARGO_MANIFEST_DIR")
        );

        println!("cargo:rustc-link-lib=Ultralight");
        println!("cargo:rustc-link-lib=UltralightCore");
        println!("cargo:rustc-link-lib=WebCore");
        println!("cargo:rustc-link-lib=AppCore");

        let profile: String = std::env::var("PROFILE").unwrap();
        let out_dir: String = std::env::var("OUT_DIR").unwrap();
        let target = format!("{}/../../../../{}/", out_dir, profile);
        let target = std::path::Path::new(&target);
        let target = target.canonicalize().unwrap();

        // If `WebCore.dll` is not found, download the archive and unzip it to the target dir.
        // WebCore.dll is to large to upload to crates.io
        let webcore_out: String = format!("{}\\WebCore.dll", target.to_str().unwrap());
        if !std::path::Path::new(&webcore_out).exists() {
            let mut response = ureq::get(
                "https://github.com/VZout/ultralight/releases/download/v0.1.5/WebCore.dll",
            )
            .call()
            .unwrap()
            .into_reader();

            //use flate2::read::GzDecoder;
            //let mut tar = GzDecoder::new(response);
            let mut file = std::fs::File::create(webcore_out).unwrap();
            std::io::copy(&mut response, &mut file).unwrap();
        }

        // Copy dll's to executable directory
        fs_extra::copy_items(
            &[
                format!(
                    "{}/ultralight-bin/Ultralight.dll",
                    env!("CARGO_MANIFEST_DIR")
                ),
                format!(
                    "{}/ultralight-bin/UltralightCore.dll",
                    env!("CARGO_MANIFEST_DIR")
                ),
                format!("{}/ultralight-bin/AppCore.dll", env!("CARGO_MANIFEST_DIR")),
            ],
            target.clone(),
            &fs_extra::dir::CopyOptions::new().skip_exist(true),
        )
        .expect("Failed to copy ultralight dlls");
    }

    #[cfg(feature = "generate_bindings")]
    {
        use std::env;

        let api_path = format!("{}/ultralight-api/", env!("CARGO_MANIFEST_DIR"));
        let out_path = format!("{}/src/generated_bindings.rs", env!("CARGO_MANIFEST_DIR"));

        let bindings = bindgen::Builder::default()
            .header(format!("{}{}", api_path, "AppCore/CAPI.h"))
            .clang_arg(format!("-I{}", api_path))
            .derive_default(true)
            .clang_arg("-Duintptr_t=unsigned __int64")
            .clang_arg("-Dintptr_t=__int64")
            .layout_tests(false)
            .rustified_enum("ULLogLevel")
            .rustified_enum("ULMessageLevel")
            .parse_callbacks(Box::new(bindgen::CargoCallbacks))
            .generate()
            .expect("Unable to generate bindings");

        bindings
            .write_to_file(out_path)
            .expect("Couldn't write bindings!");
    }
}
