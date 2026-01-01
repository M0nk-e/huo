# Huo 🔥

A fast and intuitive manga downloader CLI tool written in Rust.

## Features

- 🚀 **Fast Downloads** - Efficient async downloading with progress tracking
- 🎨 **Interactive TUI** - Beautiful terminal interface for searching and browsing
- 📚 **Multiple Sources** - Support for various manga websites
- 🔄 **Smart Updates** - Automatically tracks downloaded chapters to avoid duplicates
- 🎯 **Flexible Selection** - Download all chapters, new chapters only, or specific chapters
- 💾 **Organized Storage** - Automatically organizes downloads in your Documents folder

## Supported Sources

- **Asura Scans** (`asura`)
- **Demonic Scans** (`demonic`)
- **Flame Comics** (`flame`)

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/M0nk-e/huo.git
cd huo

# Build the project
cargo build --release

# Install globally (optional)
cargo install --path huo-cli
```

### Requirements

- Rust 1.70+ (2021 edition)
- Cargo

## Usage

### Interactive Mode

Simply run `huo` without any arguments to enter interactive mode:

```bash
huo
```

This will guide you through:
1. Selecting a manga source
2. Searching for manga or pasting a URL
3. Choosing which chapters to download

### Command Line Interface

#### Download Manga

```bash
# Download with interactive prompts
huo download --source asura --url <manga-url>

# Download specific chapters
huo download --source asura --url <manga-url> --chapters "1,2,3"

# Download chapters in a range
huo download --source asura --url <manga-url> --chapters "1-5"

# Download all chapters (including already downloaded)
huo download --source asura --url <manga-url> --all

# Skip confirmation prompts
huo download --source asura --url <manga-url> --yes
```

#### Search for Manga

```bash
# Interactive TUI search
huo search --source asura

# Direct search
huo search --source asura "manga title"
```

#### List Available Sources

```bash
huo sources
```

## Examples

```bash
# Interactive mode - easiest way to get started
huo

# Download new chapters from Asura Scans
huo download -s asura -u https://asurascans.com/manga/example/

# Download specific chapters
huo download -s demonic -u https://demonic.com/manga/example/ -c "10,11,12"

# Search for a manga
huo search -s flame "one piece"
```

## Project Structure

```
huo/
├── huo-core/          # Core library with downloader, plugins, and TUI
│   ├── src/
│   │   ├── core/      # Downloader and state management
│   │   ├── plugins/   # Manga source plugins
│   │   └── tui/       # Terminal user interface
│   └── Cargo.toml
├── huo-cli/           # CLI application
│   ├── src/
│   │   └── main.rs
│   └── Cargo.toml
└── Cargo.toml         # Workspace configuration
```

## How It Works

1. **Plugin System**: Each manga source is implemented as a plugin implementing the `MangaPlugin` trait
2. **State Tracking**: Download state is saved locally to track which chapters have been downloaded
3. **Smart Downloads**: By default, only new chapters are downloaded (unless `--all` is specified)
4. **Storage**: Manga are saved to `~/Documents/Mangas/<manga-title>/<chapter-number>/`

## Development

### Adding a New Source

To add support for a new manga source:

1. Create a new plugin file in `huo-core/src/plugins/`
2. Implement the `MangaPlugin` trait
3. Register it in `huo-cli/src/main.rs` in the `get_plugins()` function

See existing plugins for reference.

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

## License

MIT License - see [LICENSE.md](LICENSE.md) for details.

## Author

**M0nk-e** - [monkeentropy@proton.me](mailto:monkeentropy@proton.me)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Disclaimer

This tool is for personal use only. Please respect the terms of service of the manga websites you use with this tool.

