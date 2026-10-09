# DWXG Studio (Rust)

See the project-level [`README.md`](../README.md) for full compile instructions and the format specification in [`DWXG-FORMAT.md`](../DWXG-FORMAT.md).

From this directory:

```bash
cargo test
cargo run
cargo build --release
```

The app's source is in `src/main.rs` and the shared DWXG codec is in `src/format.rs`.


## Run without a terminal window (Windows)

The application is configured with `#![windows_subsystem = "windows"]`, so launching the built GUI executable does not open a separate console window. Build it with:

```powershell
cargo build --release
```

Then launch `target\release\dwxg-studio.exe` from File Explorer. Running `cargo run` from PowerShell will still leave the PowerShell window open because Cargo itself is running in that terminal; that is expected.
