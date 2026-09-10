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

fn main() {
    // Fingerprint actual build inputs, including uncommitted code. Unlike a Git
    // SHA alone, this distinguishes installed local edits and works in archives.
    let mut files = Vec::new();
    sources(Path::new("src"), &mut files);
    files.extend(
        [
            "Cargo.toml",
            "Cargo.lock",
            "build.rs",
            "assets/symphos-logo.svg",
        ]
        .map(Into::into),
    );
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
