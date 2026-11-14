# Phones and builds

**Run** (⌘R) builds the real app and opens it on the Android emulator or the iPhone simulator. **Build** (⌘B) makes an installable `.apk` or `.app` without opening anything. Both live in `app/src/device.rs`. The first-time download of the Android tools is in `app/src/android_setup.rs`.

## How a run works

`DeviceRun::run(device, project, ctx, debug)` copies the project and starts a background thread. The window never waits on it. The thread and the window share a `Shared` value behind a `Mutex`:

| `Shared` field | Meaning |
|---|---|
| `phase` | `Working` → `Running` → `Stopped`, or `Failed` |
| `step` | what Blockwise is doing, in plain words ("Building with Gradle…") |
| `lines` | the log: Blockwise's own steps, tool output, and the app's debug lines, each with a `LineKind` |
| `built_path` | where a Build put its `.apk` or `.app` |
| `setup` | Android tools that are missing, for the person to agree to download |

After changing `Shared`, the thread calls `ctx.request_repaint()`, so the window draws the new state at once. `device_dock` and `device_log` in `app.rs` draw it. **Stop** sets `cancel`, kills the log process and stops the app on the device (`StopWith`).

`debug` decides whether the generated code logs events, blocks and variable changes (see [code generation](code-generation.md#from-blocks-to-code-gen)). `parse_logcat` and `parse_console` turn those log lines back into `LineKind::Event`, `Block` and `Value`.

## Android

`run_android`:

1. `android_setup::ensure` checks for Java, the SDK, Gradle and an emulator. If anything is missing it stops and fills `setup` (see below).
2. If no emulator is running (`android_device` asks `adb devices`), it picks a virtual device (`pick_avd`) and starts it in the background, so the emulator boots while the app builds.
3. `compile_android` writes the Kotlin project into a work folder (`work_dir`) with `native::write_kotlin_project`, then runs `gradle assembleDebug`. It tries offline first and goes online only if parts are missing.
4. It waits for the emulator (`sys.boot_completed`), installs with `adb install -r`, starts the activity and follows `adb logcat` for the app's lines (`follow`).

Tools are found in the usual places, including Android Studio's (`android_sdk`, `java_home`, `gradle`, plus `which` on `PATH`). `exe(name)` adds `.exe`/`.bat` on Windows. `device::gradle_setup` reads the Gradle version so the generated build files match it.

## iPhone (macOS only)

`run_iphone`:

1. `pick_simulator` chooses a simulator (preferring the one selected in Xcode's Device Hub) and boots it with `xcrun simctl boot`.
2. `compile_iphone` writes the Swift sources (`native::write_swift_sources`) and compiles them with **`xcrun swiftc`** straight into an `.app` bundle, with no Xcode project: `-target <arch>-apple-ios17.0-simulator`, plus an `Info.plist` (`info_plist`), the resources and the icon.
3. `simctl install`, then `simctl launch --console-pty` and follow its output (`parse_console`).

On other systems `compile_iphone` returns "Building an iOS app needs a Mac with Xcode."

## First-time Android setup

A computer without Android Studio has none of the Android tools. `android_setup.rs` can fetch them: a Java (into Blockwise's folder), the SDK command-line tools, platform tools, an SDK platform, build tools, the emulator and a system image (into the folder Android Studio uses), and Gradle.

**Nothing downloads until the person agrees.** `ensure` lists what's missing as `Need`s: what, from where, about how big. If the person hasn't agreed yet, it stores them in `Shared::setup` and stops the run with an explanation. The device log shows `android_setup_card` with the list and the Android SDK license. Pressing "Download and set up" calls `android_setup::allow()` and starts the same run again. This time `ensure` calls `install`, which downloads, unpacks and creates an emulator before the build continues. An SDK already on the computer only gets its missing parts. `DeviceRun::setup_sample` shows the same card from the welcome flow, before any project has been run.

## Where the files go

- Work folders per project and device are under Blockwise's data folder (`work_dir`). Gradle and Kotlin caches stay there between runs, so a second run is fast.
- `build_out_dir(p)` is where **Build** copies the `.apk` / `.app`. `reveal_in_file_manager` shows it in Finder, Explorer or the Linux file manager.
