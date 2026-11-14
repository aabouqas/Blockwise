# The editor

The window is an [egui](https://github.com/emilk/egui) app run by eframe. egui is *immediate mode*: every frame, the code walks the model and draws it, and a click is noticed in the same call that drew the button. There is no tree of UI objects to keep in step with the model. This keeps the editor simple, but it means anything that must outlive one frame (what's selected, a drag in progress, an open dialog) is stored in a state struct.

## The pieces

```
main.rs        start-up: flags, window options, the icon
└── App (app.rs)                     the whole window; owns the Store
    ├── projects page / settings     nav_panel, projects_page, settings_page
    ├── dialogs                      new project, export, leave project…
    ├── MCP calls                    run here, on the UI thread (see mcp.md)
    └── Editor (one open project)
        ├── Design tab   designer.rs  palette, phone, inspector  → DesignState
        ├── Events tab   app.rs       the event list; opens…
        │   └── blocks editor logic.rs palette + canvas           → LogicState
        ├── Components   app.rs
        ├── Assets       app.rs, asset_view.rs, player.rs
        ├── Code page    app.rs, viewer.rs, highlight.rs
        ├── Play/Debug   runtime.rs drawn by phone.rs; debug_dock in app.rs
        └── Run/Build    device.rs, shown in device_dock
```

`App::update` (at the end of `app.rs`) runs once per frame. In order it:

1. syncs the theme and preview phone into the settings,
2. handles macOS menu commands (`menu.rs`) and MCP tool calls (`sync_mcp`, `mcp_call`),
3. draws the settings page, the open project (`editor_page`) or the projects page,
4. saves the store if it changed more than a second ago,
5. in `--screenshot` mode, counts frames, takes the picture and quits.

## The open project: `Editor`

`Editor` holds everything about the open project that isn't the project itself: the current screen and tab, the designer and blocks editor state, the running preview (`run: Option<Runtime>`), the device run, undo history, and the small flags behind dialogs (`add_screen`, `screen_delete`, `asset_rename`…).

`editor_page` draws the top bar, the tabs and the docks, then calls the tab's drawing function. Each of those returns `changed: bool` when it edited the project. Tabs receive `&mut Project` and change it directly.

### Undo and redo

Undo is snapshots, not commands. After every frame where nothing is in progress (no text field focused, no drag, no mouse button down), `editor_page` takes `project_snapshot(project)` and compares it with `ed.base`, the last settled snapshot. If they differ, the old one goes on `ed.undo` (up to 200) and `redo` is cleared. ⌘Z swaps the project for the top of `undo`.

This means any change, from any tab, a dialog or an MCP tool, is undoable without extra code. A drag or a run of typing counts as one step because snapshots wait until the edit settles.

### Saving

Changes to the project mark `App::dirty`. The store is written at most once a second, and on exit. "Automatically save" off (the default) means leaving a changed project asks first; the decision is `model::leave_rule`. Screenshot mode never saves (`no_save`).

## The Design tab: `designer.rs`

`designer::show(ui, &mut DesignState, project, screen, play)` draws three columns:

- **Insert** (left): the palette and the outline tree. Dragging a widget sets `DesignState::drag` to a `WDrag` (a new widget kind, or an existing widget being moved).
- **The phone** (middle): drawn by `phone.rs` in `Mode::Design`. The phone reports where each widget and container was drawn (`PhoneOut::rects`, `containers`), which is how a drop finds its parent and position. `put_widget` inserts the widget. While dragging near the top or bottom edge, the phone scrolls.
- **The inspector** (right): the selected widget's settings, one egui control per field.

`play: Option<Play>` is `Some` while the preview runs. The middle phone then draws in live mode and widgets can't be dragged.

## The phone: `phone.rs`

`phone.rs` draws a screen the way a phone would. The same code serves designing and playing:

- `shell_begin` / `shell_end` draw the frame of the chosen phone. The iPhone frame is drawn in code (`drawn_iphone`), with no image from Apple.
- `draw_screen` lays out the widget tree with LinearLayout rules: sizes (`Match`, `Wrap`, fixed), weights sharing the spare room, gravity, padding and margin. Scrolling rows and columns become egui `ScrollArea`s.
- `Ctx` carries the screen, the live widget states (`WState`, from the runtime), the mode, the selection and the output.
- In live mode, using a widget pushes `(widget id, event name, values)` onto `PhoneOut::events`. The editor then calls `Runtime::fire` for each one. So the phone never runs blocks itself.

`phone::sync_fonts` loads the project's font files into egui so text in the preview uses them.

## The blocks editor: `logic.rs`

`logic::show(ui, &mut LogicState, project, screen, event)` draws category chips, a palette and the canvas of one event.

- Blocks are drawn from their spec (`blocks::parse_spec`): words, typed slots (`%s`, `%d`, `%b`) and drop-downs (`%m.kind`, filled by `blocks::menu_choices`). Colours and shapes come from `blocks::CATEGORIES` and `Shape`.
- Dragging takes blocks out of the event (`take_block`, `take_one`) into `LogicState::drag`. Dropping places them at a `Target` (`place`): under a block, inside a C block, into a slot, or loose on the canvas.
- Selection (drag a box, shift-click), copy and paste (`clip_text` / `from_clip_text`: blocks as JSON on the clipboard), alt-drag to copy, and breakpoints (`LogicState::breakpoints`, a set of block ids) live here too.

## Docks: debugger and device log

While the preview runs with Debug on, `debug_dock` shows the runtime's trace, a variables view and the step/continue controls (`debug_controls` → `Runtime::debug_step`, `debug_continue`, `debug_finish`). While an app runs on a real emulator or simulator, `device_dock` shows its live log.

## Look and feel

- `theme.rs` holds Blockwise's light and dark colours (`bg`, `surface`, `ink`, `gray`…). The phone and blocks keep their own colours in both themes.
- `icons.rs` adds the Lucide icon font to egui, so an icon is a character: `lbl(ic::PLUS, "New project")`. It also draws each project's app icon.
- `PRIMARY` (in `phone.rs`) is the indigo used for main buttons and the logo.
