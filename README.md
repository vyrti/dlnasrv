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
```sh
./target/release/opendlna --media-dir /path/to/media --port 8080 --name "My DLNA Server"
```

- `--media-dir`: Directory containing your video files (default: current directory)
- `--port`: Port to listen on (default: 8080)
- `--name`: Friendly name for the server (default: "OpenDLNA")

### Access
- The server will be discoverable by DLNA/UPnP clients on your local network.
- Web interface: `http://<your-ip>:<port>/`

## Dependencies
- rust
- axum
- tokio
- serde
- uuid
- tokio-util
- thiserror
- anyhow
- clap
- tracing
- tracing-subscriber
- headers
- xml-rs

## License

This project is licensed under the [Apache License 2.0](LICENSE).
