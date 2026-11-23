# Teleport

Teleport is a native macOS file manager for Media Transfer Protocol (MTP) devices. Connect a compatible Android device over USB to browse its storage and move files between the device and your Mac.

## Features

- Detect connected MTP devices
- Browse device storage and folders
- Upload and download files and folders
- Create, rename, and delete files or folders
- Preview downloaded files and track transfer progress
- Configure app preferences and logging

## Requirements

- macOS
- Rust toolchain with the 2024 edition support
- An MTP-compatible device connected over USB

## Run

```sh
cargo run
```

## Test

```sh
cargo test
```

## Notes

Teleport uses `rusb` for USB communication and GPUI for its native interface. Device availability and supported operations depend on the device's MTP implementation.

## License

No license has been specified yet.
