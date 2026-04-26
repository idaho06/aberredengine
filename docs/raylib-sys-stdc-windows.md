# raylib-sys: `stdc++.lib` linker error on Windows (LNK1181)

## Symptom

When building with the `imgui` feature on Windows (MSVC toolchain) you get:

```
LINK : fatal error LNK1181: cannot open input file 'stdc++.lib'
```

## Root cause

`raylib-sys 5.5.1` has a bug in its build script. The `gen_imgui()` function
unconditionally requests the GCC C++ standard library regardless of platform:

```rust
// raylib-sys-5.5.1/build.rs  (line ~336)
fn gen_imgui() {
    println!("cargo:rustc-link-lib=dylib=stdc++");  // ← no platform guard
    ...
}
```

`stdc++.lib` exists only on GCC/MinGW toolchains. MSVC links the C++ runtime
automatically as a default lib and has no `stdc++.lib`. The fix is to guard that
line so it only fires on non-Windows targets.

## Why simply patching the registry source isn't enough

Cargo separates the build of a build script from its execution into **two
distinct directories** inside `target/release/build/`:

| Directory | Contains |
|-----------|----------|
| `raylib-sys-<hash-A>` | The **compiled build-script binary** (`build_script_build.exe`) |
| `raylib-sys-<hash-B>` | The **output** of running that binary (link flags, etc.) |

When the build output directory (`hash-B`) is deleted, Cargo re-runs the
already-compiled binary from `hash-A`. Because the binary was compiled from
the *unpatched* source, it still emits `stdc++`.

You must delete **both** directories — and the corresponding fingerprint
entries — to force Cargo to recompile the build script from the patched source.

## Manual fix

1. Edit `~/.cargo/registry/src/<index>/raylib-sys-<version>/build.rs` and wrap
   the offending line:

   ```rust
   fn gen_imgui() {
       let target = std::env::var("TARGET").unwrap_or_default();
       if !target.contains("windows") {
           println!("cargo:rustc-link-lib=dylib=stdc++");
       }
       // ...
   }
   ```

2. Delete both build and fingerprint cache entries for `raylib-sys`:

   ```powershell
   Remove-Item -Recurse -Force target\release\build\raylib-sys-*
   Remove-Item -Recurse -Force target\release\.fingerprint\raylib-sys-*
   ```

3. Rebuild from a **VS Developer PowerShell** session (so CMake and MSVC tools
   are on PATH):

   ```powershell
   cargo build --release
   ```

## Automated fix

Run the helper script from the project root:

```powershell
.\fix-raylib-stdc.ps1
```

See [`fix-raylib-stdc.ps1`](../fix-raylib-stdc.ps1).

## Notes

- This is a bug in `raylib-sys`. If a newer version fixes it (by guarding the
  `stdc++` link behind a non-Windows `cfg`), the script will detect the patch
  is already applied and skip it.
- The registry patch survives `cargo clean` on the project but is lost if
  Cargo re-downloads or re-extracts the crate (e.g. `cargo fetch --offline`
  after clearing `~/.cargo/registry/src/`). Re-run the script if that happens.
