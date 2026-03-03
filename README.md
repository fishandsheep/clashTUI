# mihomo-tui

A Terminal User Interface (TUI) for managing and controlling [Mihomo](https://github.com/MetaCubeX/mihomo) proxy service.

## Features

- **Proxy Group Management** - View and switch between proxy groups
- **Node Selection** - Select proxy nodes within each group
- **Real-time Latency Display** - View node latency with color coding:
  - 🟢 **Green** < 200ms
  - 🟡 **Yellow** 200-500ms
  - 🔴 **Red** > 500ms
  - ⚫ **Gray** -- Not tested/timeout
- **Mode Switching** - Switch between Rule/Global/Direct modes
- **Async Operations** - All operations run asynchronously, no UI blocking
- **Vim-style Navigation** - Use `j/k` to navigate nodes

## Requirements

- Rust 1.70+
- Mihomo proxy service running on `127.0.0.1:9090`

## Installation

```bash
# Clone the repository
git clone <repository-url>
cd mihomo-tui/tui

# Build in release mode
cargo build --release

# The binary will be at target/release/mihomo-tui
```

## Usage

```bash
# Run the TUI
cargo run --release

# Or use the built binary
./target/release/mihomo-tui
```

## Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Tab` | Switch to next proxy group |
| `↑/↓` | Select proxy group |
| `j/k` | Move up/down within nodes |
| `←/→` | Select previous/next node |
| `s` | Switch to selected node |
| `f` | Refresh all node delays |
| `r` | Switch to Rule mode |
| `g` | Switch to Global mode |
| `d` | Switch to Direct mode |

## Interface Overview

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ r:Rule g:Global d:Direct  |  Mode: RULE (Rule-based proxy)                  │
├──────────────────────────────────────────────┬─────────────────────────────┤
│  Groups [S=Selectable]                       │  [Group Name]               │
│                                              │                             │
│  [S] Group1                                  │  [✓] Node1      123ms      │
│  ★ [S] Group2                                │      Node2      456ms      │
│  [S] Group3                                  │      Node3       --       │
│                                              │                             │
├──────────────────────────────────────────────┴─────────────────────────────┤
│ q quit | Tab next group | j/k up/down node | s switch | f refresh delays   │
│ r rule | g global | d direct mode                                         │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Configuration

The application connects to Mihomo API at `127.0.0.1:9090` by default.

### Mihomo Configuration

Ensure your Mihomo configuration has the external controller enabled:

```yaml
external-controller: 127.0.0.1:9090
```

## Development

```bash
# Build
cargo build

# Run with logging
cargo run

# Check code
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```

## License

MIT License

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
