use std::{fs, path::Path};

fn sources(path: &Path, files: &mut Vec<std::path::PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read source directory") {
            sources(&entry.expect("read source entry").path(), files);
        }
    } else {
        files.push(path.to_owned());
    }
}

fn build_macos_capture() {
    cc::Build::new()
        .file("src/audio/macos_capture.m")
        .file("src/project_menu.m")
        .file("src/window_chrome.m")
        .file("src/color_picker.m")
        .file("src/midi/macos.m")
        .file("src/recording/encoder.m")
        .flag("-fobjc-arc")
        .flag("-std=c11")
        .flag("-mmacosx-version-min=14.2")
        .warnings_into_errors(true)
        .compile("symphos_capture");
    println!("cargo:rustc-link-lib=framework=CoreAudio");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=CoreMedia");
    println!("cargo:rustc-link-lib=framework=CoreVideo");
    println!("cargo:rustc-link-lib=framework=CoreMIDI");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=UniformTypeIdentifiers");
}

fn build_milkdrop() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("milkdrop");
    let cmake = std::env::var("CMAKE").unwrap_or_else(|_| {
        ["/opt/homebrew/bin/cmake", "/usr/local/bin/cmake"]
            .into_iter()
            .find(|p| Path::new(p).is_file())
            .unwrap_or("cmake")
            .into()
    });
    let run = |args: &[&std::ffi::OsStr]| {
        let status = std::process::Command::new(&cmake)
            .args(args)
            .status()
            .expect("MilkDrop needs CMake 3.21+ and a C++ compiler; install cmake or set CMAKE");
        assert!(status.success(), "MilkDrop native build failed");
    };
    run(&[
        "-S".as_ref(),
        "src/milkdrop".as_ref(),
        "-B".as_ref(),
        out.as_os_str(),
        "-DCMAKE_BUILD_TYPE=Release".as_ref(),
        "-DCMAKE_OSX_DEPLOYMENT_TARGET=14.2".as_ref(),
    ]);
    run(&[
        "--build".as_ref(),
        out.as_os_str(),
        "--target".as_ref(),
        "projectM".as_ref(),
        "--parallel".as_ref(),
        "4".as_ref(),
    ]);
    println!(
        "cargo:rustc-link-search=native={}",
        out.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=projectM-4");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=framework=OpenGL");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=EGL");
        println!("cargo:rustc-link-lib=dl");
    }
    println!("cargo:rerun-if-env-changed=CMAKE");
    println!("cargo:rerun-if-changed=vendor/projectm");
}
fn main() {
    build_milkdrop();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_macos_capture();
    }

    // Fingerprint actual build inputs, including uncommitted code. Unlike a Git
    // SHA alone, this distinguishes installed local edits and works in archives.
    let mut files = Vec::new();
    sources(Path::new("src"), &mut files);
    sources(Path::new("assets/milkdrop"), &mut files);
    sources(Path::new("vendor/projectm"), &mut files);
    files.extend(["Cargo.toml", "Cargo.lock", "build.rs"].map(Into::into));
    files.sort();
    let mut hash = 0xcbf29ce484222325_u64;
    for file in files {
        println!("cargo:rerun-if-changed={}", file.display());
        let contents = fs::read(&file).expect("read build input");
        for byte in file
            .to_string_lossy()
            .bytes()
            .chain([0])
            .chain(contents)
            .chain([0])
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rustc-env=SYMPHOS_BUILD_ID=src-{hash:016x}");
}
