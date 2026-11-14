# Testing

```bash
cd app
cargo test --release
```

This runs about 80 tests in a few seconds, on every system. Tests that need a real emulator or simulator are marked `#[ignore]` and run only when asked (below). GitHub Actions runs the normal tests on macOS, Linux and Windows for every push and pull request (`.github/workflows/build.yml`).

Tests live next to the code they test, in `#[cfg(test)] mod tests` at the bottom of each file.

## Tests never touch real data

- The store is only loaded by the app, never by tests.
- Project files go to a temporary folder: `model::files_root()` returns `temp_dir()/blockwise-test-files` under `cfg(test)` (or `BLOCKWISE_FILES_ROOT`).
- Device tests write their projects to temporary work folders.

Keep it that way: a test must never read or write `projects.json` or the real `files/` folder.

## Kinds of tests

### Model, engine and code tests

Plain Rust. Build a `Project` (or use a helper such as `app::sample_project`, `runtime`'s `every_block_project`, or the projects in `device.rs`), then:

- **Engine:** make a `Runtime`, `fire` an event, `tick` time forward, and check `vars`, `widgets`, `toasts`, `crash` or `trace`. For example, `debugger_pauses_steps_and_continues` sets a breakpoint and steps.
- **Code:** generate Kotlin, Swift or XML and look for what must (or must not) be there. `every_block_has_native_code` makes sure every block in `DEFS` has code in both languages.
- **Web and Supabase:** tests start a tiny HTTP server on `127.0.0.1` (`serve_once`, the pretend Supabase) so nothing goes over the internet.

### UI tests (headless egui)

egui can run without a window: give `Context::run` a `RawInput` and it lays out a frame and returns the shapes it would paint. UI tests use that to click and read the screen:

```rust
let ctx = egui::Context::default();
let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 700.0))), events, ..Default::default() };
let out = ctx.run(input, |ctx| { /* draw the page */ });
// out.shapes holds every text shape with its position
```

- **A click takes three frames:** move the pointer, press, release (see `open_and_pick` in `app.rs`).
- **Read the screen** by collecting `epaint::Shape::Text` shapes and their rectangles (`texts`), then assert on the text and where it is.
- **Scrolling** is an `egui::Event::MouseWheel` with `MouseWheelUnit::Point`.
- **Time-based behaviour** (animations, auto-scroll while dragging) needs `RawInput::time` moving forward each frame.

Examples: `screen_picker_has_an_x_to_delete` (`app.rs`), `a_long_screen_scrolls_while_designing` and `a_full_scrolling_column_plays_without_a_gap` (`designer.rs`).

## Screenshots

```bash
./target/release/Blockwise --screenshot out.png --open <scene>
```

This opens the window without taking focus, sets up a named **scene**, draws about 120 frames, saves a PNG of the window and quits. It uses a throwaway store with the sample project and never saves. Scenes are the `--open` cases in `App::new` (`app.rs`), for example `projects`, `design`, `design-play`, `logic`, `debug`, `assets`, `settings`, `source`, `kotlin`, `swift`, `android-setup`, `scroll-column`.

Add `BLOCKWISE_DARK=1` for the dark theme and `BLOCKWISE_PREVIEW=android` for the Android phone. Use screenshots in pull requests that change the UI, and to refresh `docs/images/`.

## The phone checks

`app/src/check.rs` answers one question: **does the engine run blocks the way the phones do?** Each check builds a block program that stores every result in a variable. It runs the program in the engine, then as a debug build on the Android emulator and on the iPhone simulator. Debug builds log every value set, so the three lists can be compared value by value.

```bash
cd app
cargo test --release check_ -- --ignored --nocapture                      # all checks, both phones
BLOCKWISE_DEVICE=android cargo test --release check_preview_against_phones -- --ignored --nocapture
```

| Check | Covers |
|---|---|
| `check_preview_against_phones` | numbers, text, lists, maps, JSON, loops |
| `check_crashes_against_phones` | mistakes that crash, one small app each |
| `check_timers_against_phones` | repeating, cancelled and restarted timers |
| `check_supabase_against_phones` | Supabase calls, against a pretend Supabase on this computer |

Run these after changing how a block behaves in `runtime.rs` or `native.rs`.

## End-to-end device tests

`device.rs` has sample apps (`inputs_project`, `tap_crash_project`, `nav_project`…) that build, install and run on a device:

```bash
BLOCKWISE_E2E=inputs cargo test --release iphone_toast_and_navigation -- --ignored --nocapture
BLOCKWISE_E2E=inputs BLOCKWISE_DEVICE=android cargo test --release iphone_toast_and_navigation -- --ignored --nocapture
```

To read the generated code of a test project, run the ignored writers with `BLOCKWISE_NATIVE_OUT=<folder>` (for example `write_every_block_native`).
