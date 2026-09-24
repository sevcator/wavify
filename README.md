# Wavify

Wavify is a Windows desktop music player built with Rust and Iced.

## Download

Download `Wavify-Setup.exe` from the [latest GitHub release](https://github.com/sevcator/wavify/releases/latest). The installer is per-user and installs the Microsoft Edge WebView2 Runtime when it is missing.

## Build from source

The release build targets 64-bit Windows GNU and uses the nightly Rust toolchain configured in `.cargo/config.toml`.

```powershell
rustup toolchain install nightly --component rust-src
rustup target add x86_64-pc-windows-gnu --toolchain nightly
$env:WAVIFY_GITHUB_REPOSITORY = "OWNER/Wavify"
cargo +nightly build --release --target x86_64-pc-windows-gnu --bin wavify
```

The release workflow builds the application, packages it with Inno Setup, and publishes the installer. Push a `v*` tag or run **Build and publish release** from GitHub Actions with a tag matching the version in `Cargo.toml`.

## Updates

Wavify checks the latest GitHub release when it starts. Update checks can be disabled in Settings, and **Check updates now** always remains available. Update notes come from `CHANGELOG.md` in the release workflow.
