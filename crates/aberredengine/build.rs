// build.rs

#[cfg(windows)]
fn main() {
    let _ = embed_resource::compile("aberred.rc", embed_resource::NONE);
}

#[cfg(unix)]
fn main() {}
