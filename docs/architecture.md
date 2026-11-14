# How Blockwise is built

A one-page map of the code for contributors. For how each part works, see the [developer guide](developer/README.md). Blockwise is one Rust crate: a desktop app on [egui](https://github.com/emilk/egui) / eframe. It has no web view and no server. It keeps your projects in one JSON file and turns them into native Kotlin and SwiftUI code.

## Folders

| Folder | What's in it |
|---|---|
| `app/` | The desktop app: `Cargo.toml`, the code in `src/`, and `assets/` (the icon font and icons built into the app) |
| `brand/` | The logo, the wordmark and the app icon, as SVG |
| `docs/` | These guides, and the README's screenshots in `docs/images/` |
| `licenses/` | License texts of the third-party parts Blockwise includes |

## The big picture

```
            ┌──────────────┐
            │  model.rs    │  Project → Screens → Widgets, Variables,
            │  (the data)  │  Components, Events → Blocks
            └──────┬───────┘
     ┌─────────────┼──────────────┬──────────────────┐
     ▼             ▼              ▼                  ▼
 designer.rs   logic.rs      runtime.rs        codegen.rs / native.rs
 (screens)     (blocks)      (runs blocks      (Kotlin + XML, SwiftUI)
     │             │          in the editor)          │
     └──── app.rs (window, tabs, dialogs) ───┘         ▼
                   │                               device.rs
               phone.rs (draws the phone)          (Gradle / swiftc,
                                                   emulator / simulator)
```

One project model feeds three things:

1. **The editor** draws and edits the project.
2. **The runtime** runs its blocks inside the editor (Play and Debug).
3. **The code generators** write the same app for Android and iOS.

`check.rs` keeps these three in agreement.

## Modules

All in `app/src/`.

| File | What it does |
|---|---|
| `main.rs` | Start-up and command-line flags (`--mcp`, `--screenshot`, `--open`) |
| `model.rs` | The data: projects, screens, widgets, variables, components, events, blocks; loading and saving |
| `app.rs` | The window: projects page, editor tabs, dialogs, settings, the run/debug bar and dock |
| `designer.rs` | The Design tab: widget palette, the phone you drop widgets on, the inspector |
| `phone.rs` | Draws widgets as a phone would, both static (designing) and live (playing); the phone frames |
| `logic.rs` | The blocks editor: palette, canvas, snapping, selection, breakpoints |
| `blocks.rs` | The block catalogue: categories, colours, shapes, labels |
| `runtime.rs` | Runs a project's blocks the way the generated Android app would: Play and Debug |
| `codegen.rs` | Shared code generation: the block walker, layout XML, export |
| `native.rs` | Kotlin (Android) and SwiftUI (iOS) code for every block and widget, plus each project's helper code |
| `device.rs` | Builds and runs apps on the Android emulator (Gradle) and the iPhone simulator (swiftc + simctl) |
| `android_setup.rs` | First-run download of Java, the Android SDK, Gradle and an emulator, with consent |
| `mcp.rs` | The MCP server for AI assistants, and the `--mcp` stdio bridge |
| `templates.rs` | Ready-made screens (log in, sign up, settings, list, Supabase log in) |
| `examples.rs` | The example apps built into Blockwise from `examples/`, and adding a copy to the projects |
| `asset_view.rs`, `player.rs` | The Assets page's viewer and media player (AVFoundation on macOS) |
| `viewer.rs`, `highlight.rs` | The code page and build log: settings and syntax colours |
| `menu.rs` | The macOS menu bar |
| `theme.rs`, `icons.rs` | Colours for light and dark, and the Lucide icon font |

## Ideas worth knowing

- **One engine, two phones.** A block's meaning is set by `runtime.rs`. `native.rs` writes Kotlin and Swift that behave the same way: numbers are Doubles everywhere, and errors crash the same way. `check.rs` runs the same block programs in the engine and on real devices and compares the results.
- **Undo is snapshots.** Edits replace the project; undo and redo keep earlier copies.
- **Tests draw real frames.** UI tests run egui headless (`Context::run` with `RawInput`), click by position over several frames, and read painted text shapes.
- **Screenshots.** `Blockwise --screenshot out.png --open <scene>` renders a named scene (see the `--open` match in `app.rs`). It is used for docs and for checking UI changes.

## Going deeper

| Guide | Covers |
|---|---|
| [The data model](developer/data-model.md) | projects, screens, widgets, blocks, saving |
| [The editor](developer/editor.md) | the window, the designer, the blocks editor, undo |
| [The engine](developer/engine.md) | running blocks, crashes, timers, the debugger |
| [Code generation](developer/code-generation.md) | Kotlin, Swift, XML, export |
| [Phones and builds](developer/devices.md) | Gradle, swiftc, emulator, simulator, Android setup |
| [The MCP server](developer/mcp.md) | AI assistants |
| [Testing](developer/testing.md) | unit, UI, screenshot and phone tests |
| [Recipes](developer/recipes.md) | adding blocks, settings, widgets, events, tools |
| [Releasing](developer/releasing.md) | tags, downloads, signing |

## Building and testing

Run these in the `app/` folder.

```bash
cd app
cargo build --release
cargo test --release
```

Some tests drive real devices and are ignored by default. They need the emulator or simulator:

```bash
# build and run a sample app on the iPhone simulator
BLOCKWISE_E2E=inputs cargo test --release iphone_toast_and_navigation -- --ignored --nocapture

# the same on the Android emulator
BLOCKWISE_E2E=inputs BLOCKWISE_DEVICE=android cargo test --release iphone_toast_and_navigation -- --ignored --nocapture

# compare the engine with both phones, block by block
cargo test --release check_preview_against_phones -- --ignored --nocapture
```

Tests write projects and files to temporary folders, never to your real Blockwise data.
