# OpenDLNA Media Server

A lightweight, async DLNA/UPnP media server written in Rust using Axum and Tokio. Designed for streaming video files (MKV, MP4, AVI) from a local directory to DLNA/UPnP clients on your network.

## Features
- Serves video files over DLNA/UPnP
- SSDP discovery for client compatibility
- HTTP streaming with range support
- Dynamic XML generation for device/service description
- Configurable via command-line (media directory, port, server name)
- Async, efficient, and cross-platform

## Usage

### Build
```sh
cargo build --release
```

### Run
Usage: opendlna [OPTIONS] [MEDIA_DIR]

Arguments:
  [MEDIA_DIR]  The directory containing media files to serve [default: /Users/alex/Documents/code/rust/dlnarust]

Options:
  -p, --port <PORT>  The network port to listen on [default: 8080]
  -n, --name <NAME>  The friendly name for the DLNA server [default: "Rust DLNA Server"]
  -h, --help         Print help
  -V, --version      Print version

## License

This project is licensed under the [Apache License 2.0](LICENSE).
