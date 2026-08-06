// build.rs

#[cfg(windows)]
fn main() {
    let macros = [
        format!("VERSION_MAJOR={}", env!("CARGO_PKG_VERSION_MAJOR")),
        format!("VERSION_MINOR={}", env!("CARGO_PKG_VERSION_MINOR")),
        format!("VERSION_PATCH={}", env!("CARGO_PKG_VERSION_PATCH")),
        format!("VERSION_STR=\"{}\"", env!("CARGO_PKG_VERSION")),
    ];
    embed_resource::compile("aberred.rc", &macros)
        .manifest_optional()
        .unwrap();
}

#[cfg(unix)]
fn main() {}
