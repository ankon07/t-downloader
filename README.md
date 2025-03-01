# Video Downloader

A versatile media downloader application built in Rust that supports downloading videos and audio from YouTube and other platforms.

## Features

- Download videos in various formats and qualities
- Extract audio from videos
- Command-line interface with interactive prompts
- Support for custom output directories
- Progress bar display during downloads
- Format selection with resolution and file size information

## Prerequisites

Before running the application, ensure you have:

1. Rust and Cargo installed (https://rustup.rs/)
2. yt-dlp installed (the application will attempt to install it if missing)

## Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/ankon07/t-downloader.git
   cd video-downloader
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

The compiled binary will be available in `target/release/video-downloader`

## Usage

### Basic Usage

1. Run the application:
   ```bash
   cargo run
   ```

2. When prompted, enter the video URL and follow the interactive prompts to select:
   - Download type (video/audio)
   - Format and quality
   - Output directory

### Command Line Arguments

You can also use command-line arguments:

```bash
video-downloader --url <URL> --output-dir <PATH> [--verbose]
```

Options:
- `-u, --url`: Specify the video URL directly
- `-o, --output-dir`: Set the output directory for downloaded files
- `-v, --verbose`: Enable verbose logging
- `-h, --help`: Display help information
- `-V, --version`: Show version information

## Dependencies

The application uses several Rust crates:
- clap: Command-line argument parsing
- tokio: Async runtime
- reqwest: HTTP client
- indicatif: Progress bars
- dialoguer: Interactive CLI
- and more (see Cargo.toml for full list)

## License

This project is open source and available under the MIT License.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
