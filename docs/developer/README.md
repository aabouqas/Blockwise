# Developer guide

Everything a developer needs to understand Blockwise's code and start changing it. Read [How Blockwise is built](../architecture.md) first for the one-page map, then the guide for the part you want to work on.

| Guide | Read it to… |
|---|---|
| [The data model](data-model.md) | understand projects, screens, widgets, blocks and how they are saved |
| [The editor](editor.md) | change the window: the projects page, the designer, the blocks editor, undo |
| [The engine](engine.md) | change how blocks run in Play and Debug, and how the debugger pauses |
| [Code generation](code-generation.md) | change the Kotlin, Swift and XML Blockwise writes |
| [Phones and builds](devices.md) | change how apps are built and run on the emulator and simulator |
| [The MCP server](mcp.md) | change what AI assistants can do |
| [Testing](testing.md) | run, write and debug tests, and make screenshots |
| [Recipes](recipes.md) | add a block, a widget setting, a widget, an event, an MCP tool or a template, step by step |
| [Releasing](releasing.md) | make a release with downloads for macOS, Windows and Linux |

## Getting set up

You need [Rust](https://rustup.rs) (stable). Everything else is a Cargo dependency.

```bash
git clone https://github.com/aabouqas/Blockwise.git
cd Blockwise/app
cargo run --release
```

Debug builds work too (`cargo run`). Dependencies are always optimised (see `[profile.dev]` in `app/Cargo.toml`), so a debug build of Blockwise still runs smoothly.

To try apps on phones you also need:

- **Android:** nothing. The first Run offers to download Java, the Android SDK, Gradle and an emulator ([Phones and builds](devices.md#first-time-android-setup)).
- **iOS:** a Mac with Xcode and an iOS simulator.

## Where things are on disk

| What | Where (macOS; other systems use their data folder) |
|---|---|
| All projects and settings, in one JSON file | `~/Library/Application Support/Blockwise/projects.json` |
| Each project's pictures, sounds, fonts, videos | `~/Library/Application Support/Blockwise/files/<project id>/` |
| Build folders for the emulator and simulator | inside the same folder, per project and device |
| Exported source | `~/Blockwise/` unless changed in Settings |

Tests never touch these: they use temporary folders (see [Testing](testing.md)).

## The rules of the code base

- **One crate, plain Rust.** No async runtime, no web view, no server. Background work (builds, web requests, the MCP listener) runs on `std::thread`s and hands results to the UI thread.
- **One model, three readers.** The editor, the engine and the code generators all read the same `Project` (see [the data model](data-model.md)). If a block or widget behaves differently in Play than on a phone, that is a bug.
- **Immediate mode UI.** The window is redrawn from the model every frame with egui. There are no widgets objects to keep in sync; state that must survive a frame lives in `App`, `Editor`, `DesignState` or `LogicState`.
- **Plain words in the UI.** "Text field", not "EditText"; "Couldn't reach the server", not "HTTP error". Comments are written the same way, for people who may not be Android or iOS experts.
- **No warnings.** `cargo build` must stay warning-free.

## Environment variables

These change behaviour for testing and screenshots. None are needed to use Blockwise.

| Variable | Effect |
|---|---|
| `BLOCKWISE_FILES_ROOT` | Folder for project assets instead of the real one |
| `BLOCKWISE_DARK=1` | Screenshots in the dark theme |
| `BLOCKWISE_PREVIEW=android` | Screenshots with the Android phone (default iPhone) |
| `BLOCKWISE_SHOT_FRAMES` | Frames to draw before a screenshot is taken (default 120) |
| `BLOCKWISE_DEVICE=android` / `ios` | Which phone device tests use |
| `BLOCKWISE_E2E=<name>` | Which sample app the end-to-end device test runs |
| `BLOCKWISE_NATIVE_OUT` | Where the ignored "write the code" tests write their output |
| `BLOCKWISE_NO_IOS` | Skip the iPhone half of the phone checks |
