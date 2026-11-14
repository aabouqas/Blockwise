# Code generation

Blockwise turns a `Project` into two native apps: an Android Studio project (Kotlin + XML layouts) and SwiftUI sources for Xcode. The code is written to be read: someone who exports a project should be able to open it and keep working by hand.

| File | Holds |
|---|---|
| `app/src/codegen.rs` | `Gen`, the walker that turns blocks into code for either language; the layout XML; `export` |
| `app/src/native.rs` | the code of each block in Kotlin and Swift (templates), each screen's Kotlin activity and SwiftUI view, the helper code every app carries, the Android manifest and Gradle files, and the writers |

## From blocks to code: `Gen`

`Gen::new(screen, lang)` walks blocks for one screen in one `Lang` (`Kotlin` or `Swift`). `event_body(event)` returns an event's code; `block(b)` returns one block's.

Every block has a **template** per language in `native.rs`: `kotlin_tpl(op)` and `swift_tpl(op)`. Templates are strings with placeholders:

| Placeholder | Becomes |
|---|---|
| `{0}`, `{1}`, … | the block's slots, in the order of its spec |
| `{S1}`, `{S2}` | the inner stacks of a C or if/else block |
| `{R}` | a fresh loop counter name for `repeat` (nested repeats get different names) |

For example `setText` is `{0}.setText({1})` in Kotlin and `{0}.text = {1}` in Swift, and `repeat` is `for ({R} in 0 until ({0}).toInt()) {\n{S1}\n}` in Kotlin. `Gen::arg` fills a slot. A typed value becomes a literal (numbers always as `Double`s, `num_literal_for`). A plugged-in reporter becomes its own code, in brackets when needed. A drop-down becomes the name picked.

The walker also tracks context the templates can't see. Inside a web request's answer stack (`web_depth`), `webResponse` reads the answer, while outside it gives `""` as the engine does. The same goes for `mediaPath` inside camera blocks (`media_depth`) and a More Block's inputs (`params`). A block with no template becomes a comment saying it's "not available", and a test fails if any block lacks one (see below).

**Debug builds.** With `trace` on, the generated code logs every event, block and variable change under the tag `Blockwise` (`LOG_TAG`). The device log shows these lines, and the phone checks compare them with the engine's `values`.

## Layouts

**Android.** `codegen::xml(screen)` writes the screen's layout XML: one `LinearLayout`, `ScrollView`, `Button`… per widget (`WKind::android_class`), with sizes, gravity, weight, colours and text as attributes. Rounded corners, shadows and borders, which XML can't say for every view, are set from Kotlin when the activity starts.

**iOS.** SwiftUI has no LinearLayout, so `native.rs` ships one: the `Linear` layout in the helper code measures and places children with the same rules (match, wrap, fixed sizes, weights, gravity). `swift_widget` writes each widget as a SwiftUI view inside it. Each screen gets a view and an `ObservableObject` model that holds the widgets' live state (`WState`), its variables and lists, and the events as methods.

## A screen's code

| | Android (`kotlin_with`) | iOS (`swift_screen_with`) |
|---|---|---|
| File | `<Name>Activity.kt` + `res/layout/<name>.xml` | `<Name>Screen.swift` |
| State | fields on the activity | a model class |
| Events | listeners set in `onCreate` (`kotlin_listener`) | methods called from the views |
| Navigation | `Intent`s with extras | a navigation path of `Route`s |

## The helper code

Things both phones need but the platforms don't offer directly are written once per app:

- **Android: `kotlin_util` → `AppUtil.kt`:** toasts, random numbers, dp conversion, the password field's eye, saved data, sound and video helpers, JSON. `kotlin_crash_catcher` and `kotlin_crash_activity` give every app the crash screen. `supa_kotlin` is the Supabase client, included only when the project uses Supabase.
- **iOS: `swift_helpers` → `Helpers.swift`:** `Linear`, `PasswordField`, `PictureView`, `VideoBox`, `WebPage`, toasts, `Prefs`, the `Supa` client, and more.

## Writing it out

- `write_kotlin_project(p, dir, gradle, trace)` writes a whole Gradle project (`gradle_files`: settings, build files, wrapper properties matched to the Gradle on this computer by `device::gradle_setup`), the manifest (`kotlin_manifest`), activities, layouts, resources, icons and the project's files.
- `write_swift_sources(p, dir, trace)` writes `App.swift` (`swift_app`), one file per screen, `Helpers.swift`, and the files in `Resources/`.
- `export_native(p, base, kotlin, swift)` uses both for **Export source**: `android-kotlin/` and `ios-swift/` (with the icon's `Assets.xcassets` and a `HOW-TO-OPEN.txt`). `codegen::export` adds a folder named after the project.

The code page (⌘J) shows the same output, live, for the current screen.

## Keeping the two phones and the engine in step

A block's meaning is decided by the engine (`runtime.rs`). Kotlin and Swift must behave the same way:

- **Numbers are `Double`** in all three. Integers print without `.0`, and rounding follows Java.
- **Mistakes crash** the same way: an out-of-range list index crashes the preview, the Android app and the iOS app.
- Tests guard this. `every_block_has_native_code` (in `runtime.rs`) builds a project using every block in `DEFS` and fails if either language says "not available" or "unknown block", or has unbalanced braces. The [phone checks](testing.md#the-phone-checks) run the same programs on real emulators and compare every value.
