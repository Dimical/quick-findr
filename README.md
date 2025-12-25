# QuickFindr

QuickFindr is a fast, lightweight Windows desktop search tool built with **Rust** + **Slint**.
It helps you scan a folder for filenames (and optionally file contents) with a modern UI, filters, and a favorites/recents system.

## Features

- Fast multithreaded scanning (Rayon + ignore walker)
- Filename search
- Optional content search (line excerpt shown on match)
- Case-sensitive toggle
- Regex mode (and wildcard support: `*` / `?`)
- Respect `.gitignore` (optional)
- Exclude extensions (comma-separated)
- Favorites & recent folders (persisted to disk)
- Quick actions: open file, reveal in Explorer, copy paths

## Requirements

- Windows 10/11
- Rust toolchain (stable)

## Install / Run (development)

```bash
cargo run
```

## Usage

1. Choose a folder (or keep the default one).
2. Enter a query.
3. Configure options:
   - Case sensitive
   - Regex / wildcards
   - Search content
   - Respect `.gitignore`
   - Exclude extensions (e.g. `.exe,.dll,.png`)
4. Click **Scan**.

## Favorites & Recents

- Add current folder to favorites via the favorites menu.
- Remove a favorite using the trash icon.
- Selecting a favorite/recent updates the current search folder.

### Persistence location

Favorites and recents are stored as JSON under your config directory:

- **Windows**: `%APPDATA%\quick-findr\favorites.json`

(Internally this uses `dirs::config_dir()`.)

## Project structure

- `src/main.rs`
  - UI setup and Slint callback bindings
  - Thread-local models used by Slint
- `src/engine.rs`
  - Search engine (walker + optional content scan)
  - Sends results back to the UI in batches
- `src/favorites.rs`
  - Favorites/recents persistence (load/save JSON)
- `ui/app_window.slint`
  - UI layout and components
- `assets/icon.png`
  - Application icon

## Notes about the Windows icon

The build script (`build.rs`) attempts to embed the icon via `winres` on Windows.
Depending on your setup, embedding a `.png` may not always be supported by the resource compiler.
The application will still run if embedding fails (a warning will be printed during build).

## Contributing

- Format code:
  
  ```bash
  cargo fmt
  ```

- Run:
  
  ```bash
  cargo run
  ```

## License

TBD
