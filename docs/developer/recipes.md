# Recipes

Step-by-step checklists for the changes people make most. Paths are in `app/src/`. For widget kinds, Rust's exhaustive `match` helps: add the new variant and the compiler lists most of the places that need it.

- [Add a block](#add-a-block)
- [Add a widget setting](#add-a-widget-setting)
- [Add a widget kind](#add-a-widget-kind)
- [Add a widget event](#add-a-widget-event)
- [Add an MCP tool](#add-an-mcp-tool)
- [Add a screen template](#add-a-screen-template)
- [Add a screenshot scene](#add-a-screenshot-scene)

## Add a block

Say you want a statement block that shakes a widget for a number of milliseconds.

1. **Define it** in `blocks.rs`: add a `b!(…)` line to `DEFS` with a unique `op`, its category index (see `CATEGORIES`), its `Shape` (`Stmt`, `C`, `E`, `Cap`, or a reporter: `Num`, `Str`, `Bool`) and its spec. In the spec, `%s` is a text slot, `%d` a number, `%b` true/false, and `%m.<kind>` a drop-down whose choices come from `menu_choices` (widgets of a kind, variables, components, screens, colours…).
   ```rust
   b!("shakeView", 6, Stmt, "shake %m.view for %d ms"),
   ```
2. **Put it in the palette**: add the `op` to its category's list in `blocks::palette` (categories 3 and 4 list everything automatically). `"#Heading"` entries are section titles.
3. **Make it run** in `runtime.rs`: an arm in `exec_inner` for a statement, or in `eval` for a reporter. Read slots with `txt`, `num`, `boolean` and `name`. Call `crash(…)` where the phone would throw.
4. **Write its code** in `native.rs`: an entry in `kotlin_tpl` and in `swift_tpl`, using `{0}`, `{1}`… for slots and `{S1}`/`{S2}` for inner stacks. If it needs more than one line of platform code, add a function to the helper code (`kotlin_util` / `swift_helpers`) and call it from the template. Remember numbers are `Double`.
5. **Test it:**
   - `cargo test` runs `every_block_has_native_code`, which fails until both templates exist.
   - Add a small engine test in `runtime.rs` (fire an event, check the result).
   - If it computes something, add it to the program in `check.rs` and run `check_preview_against_phones` on both phones.
6. **Assistants** see it automatically: `list_blocks` (MCP) is built from `DEFS`.
7. **Docs:** if it's a new kind of thing, mention it in `docs/user-guide.md`.

## Add a widget setting

Say text gets a "line height" setting.

1. **Model** (`model.rs`): add the field to `Widget` with `#[serde(default)]` (or `#[serde(default = "…")]`) so old projects still load. Set its default in `Widget::new`.
2. **Inspector** (`designer.rs`): add a control for it in the properties panel, next to similar settings.
3. **Preview** (`phone.rs`): draw it in `draw_screen` / `draw_widget`. If blocks can change it while the app runs, add it to `WState` in `runtime.rs` and to `initial_state`, then read the live value from the state.
4. **Android** (`codegen.rs`, `native.rs`): an XML attribute in `xml_widget`, or a line in the activity's setup in `kotlin_with` if XML can't say it.
5. **iOS** (`native.rs`): in `swift_widget` (and `swift_wstate` if it's live state).
6. **MCP** (`mcp.rs`): read it in `widget_json`, write it in `apply_props`, and add its name to `PROPS`.
7. **Test** the code it generates on both phones (see `text_fields_bring_up_their_keyboard_on_both_phones` in `codegen.rs` for the pattern), then look at it with `--screenshot` and, ideally, on a device.

## Add a widget kind

1. **Model:** add a variant to `WKind` and fill in every method on it: `palette_name`, `android_class`, `id_prefix`, `events`, and `is_container`, `has_text`, `is_compound` where they apply. Add it to `WKind::WIDGETS` (or `LAYOUTS`) so it shows in the palette.
2. **Build:** `cargo build` now points at every `match` that needs the new kind, for example `designer::kind_icon` and the drawing in `phone.rs` and `native.rs`.
3. **Preview:** draw it in `phone.rs`, both designing and live. Give it an `initial_state` in `runtime.rs`.
4. **Code:** XML in `codegen::xml_widget`, Kotlin setup and listeners in `native.rs`, a SwiftUI view in `swift_widget` (with a helper view in `swift_helpers` if SwiftUI has nothing close).
5. **Blocks:** if blocks can pick it, add it to the drop-down kinds in `blocks::menu_choices`.
6. **MCP:** add it to `KINDS` in `mcp.rs` with a friendly name.
7. **Tests:** add it to `every_block_project` in `runtime.rs` if blocks use it, and write a test that its Kotlin and Swift are generated.

## Add a widget event

1. Add the event name to the kind's list in `WKind::events` and words for it in `model::event_words` ("is held down"). The Events tab then offers it.
2. In `phone.rs`, push it onto `PhoneOut::events` when the widget is used in the live phone, with any event values (`HashMap` of `_name` → `Val`).
3. If it carries a value, add a reporter block that reads it (like `eventPosition` reads `_position`) and support it in the engine and both templates.
4. In `native.rs`, attach a listener in `kotlin_listener` and the matching SwiftUI modifier in `swift_widget`, each calling the event's code.
5. Test that the engine runs it (`fire`) and that both languages contain the listener.

## Add an MCP tool

1. **Describe it** in `TOOLS` (`mcp.rs`): name, title, a description the assistant will read, the input schema, required fields, and whether it only reads. Keep the description concrete. It is the tool's whole manual.
2. **Implement it.** If it works on the project data only, add an arm to `project_tool` (or `store_tool` if it isn't about one project). If it needs the window (screenshots, devices, opening a project), add it to `App::mcp_call` in `app.rs`.
3. **Errors** are plain sentences for the assistant to relay: `Err("…")`.
4. **Test** it in `mcp.rs`'s tests by calling `call(&mut store, "your_tool", json!({…}))` and checking the result and the store.
5. **Document** it in the table in `docs/ai-assistants.md`.

## Add a screen template

Templates (`templates.rs`) are written in the same JSON the MCP tools take. Add a `Template` to `TEMPLATES` with an `id`, a name, a description, a suggested screen name, an icon, and a `spec` returning `{"widgets": […], "vars": […], "components": […], "events": […]}`. The helpers at the top of the file (`heading`, `field`, `toast`, `set_text`…) keep specs short. It appears in **+ Screen** and in the MCP `add_screen` tool. Add a test that applying it gives a screen with the expected widgets and that its blocks run.

## Add a screenshot scene

In `App::new` (`app.rs`), add a case to the `--open` match that sets up the window: which tab, which screen, what's selected, an open dialog. Then:

```bash
./target/release/Blockwise --screenshot /tmp/scene.png --open your-scene
```

If the scene opens a project page, make sure it isn't in the list of scenes that leave the editor closed (the `matches!` at the end of the scene setup).
