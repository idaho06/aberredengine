# Compiling in Windows
## Requirements
- [Microsoft Visual Studio Community edition](https://visualstudio.microsoft.com/downloads/)
    - CMake for Windows
    - Latest Windows SDK
    - Git for Windows
- [LLVM for Windows](https://github.com/llvm/llvm-project/releases)
    - Configure PATH
- [Rust](https://forge.rust-lang.org/infra/other-installation-methods.html#standalone-installers)
    - Configure PATH

## Compile
Open a Developer PowerShell session in the terminal. This configures all development tools (CMake) in the $PATH.

Then you can run the usual:
```powershell
cargo build --release
```

## Distribution
TODO: Instructions for creating a `msi`, `zip` or `install.exe` distributable