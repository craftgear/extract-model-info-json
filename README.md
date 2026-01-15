# extract-model-info-json

CLI tool to scan a directory tree, find zip files alongside safetensors, and extract `model_info.json` from those zips.

## Features

- Recursively scans a root directory
- Looks for zip files in directories that contain at least one `.safetensors` file
- Extracts only `model_info.json` if present in the zip
- Overwrites existing `model_info.json` in the same directory
- Shows progress in the terminal

## Requirements

- Rust toolchain (stable)

## Build

```sh
cargo build --release
```

## Usage

```sh
./target/release/extract-model-info-json /path/to/root
```

Progress is printed to stderr. A summary is printed to stdout.

## Tests

```sh
cargo test
```

## Output behavior

- Extraction target is the same directory as the zip file
- If multiple `model_info.json` entries exist in a zip, the first match is extracted
- Existing `model_info.json` files are overwritten
