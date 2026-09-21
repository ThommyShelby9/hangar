use std::path::PathBuf;

fn main() {
    // SDL3 (controllers) is linked dynamically against the official prebuilt library in vendor/sdl3.
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let sdl = manifest.join("..").join("vendor").join("sdl3");
    println!("cargo:rustc-link-search=native={}", sdl.display());
    println!("cargo:rerun-if-changed={}", sdl.join("SDL3.dll").display());

    // SDL3.dll must sit next to the executables: target/<profile> (app) and target/<profile>/deps (tests).
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    if let Some(profile_dir) = out.ancestors().nth(3) {
        for dir in [profile_dir.to_path_buf(), profile_dir.join("deps")] {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::copy(sdl.join("SDL3.dll"), dir.join("SDL3.dll"));
        }
    }

    tauri_build::build()
}
