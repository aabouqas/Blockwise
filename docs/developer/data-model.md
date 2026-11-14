# The data model

Everything Blockwise knows about an app is one Rust value: a `Project`. It lives in `app/src/model.rs`, is serialised with serde, and is read by the editor, the engine and the code generators alike.

```
Store                         projects.json: every project + settings
└── Project                   one app: name, package, icon, files, Supabase link
    ├── files: Vec<ProjFile>  pictures, sounds, fonts, videos, data files
    └── screens: Vec<Screen>  one Activity on Android, one view on iOS
        ├── widgets: Vec<Widget>      the layout, as a flat list with parent ids
        ├── vars / lists              variables and lists the blocks use
        ├── components                timers, pop-ups, web requests, sound players…
        └── events: Vec<Event>        "when button1 is clicked" + its blocks
            └── blocks: Vec<Block>    a stack; a Block can hold more blocks
```

## Store and settings

`Store` is the whole file: the projects, the next project id, the theme and `Settings` (preview phone, export folder, MCP on/off and port, and so on). `Store::load` reads it at start-up; `Store::save` writes it atomically (to `projects.json.tmp`, then renames). The app saves about once a second while something changed (`App::dirty`).

New fields must load from older files, so every field added after the first version has `#[serde(default)]` (or a default function). Never rename or remove a field without keeping the old name readable.

## Screens and widgets

A `Screen` has a `name` (`"main"` becomes `main.xml` and `MainActivity.kt`), an optional title, an `app_bar` (`AppBar`: hidden, bar and title colours where 0 means the phone's own, no back arrow, and `actions`: icon buttons whose `id` is an event target like a widget's), `sheet` (`Sheet::No`, `Half` or `Full`: opens as a bottom sheet; `Project::sheet_of` keeps the first screen a page), and five lists.

**Widgets are a flat list.** Each `Widget` has an `id` (`button1`) and a `parent` id (`"root"` for the top level). The order of the list is the order children appear in their parent. Helpers on `Screen`: `widget(id)`, `children(parent)`, `is_descendant`, `remove_widget` (removes the subtree), `next_widget_id(kind)`.

`WKind` is the widget type. Its methods say what the type is called in the palette, which Android class it becomes, its id prefix, whether it holds children (`is_container`) and which events it has (`events()`):

| Kind | In the palette | Events |
|---|---|---|
| `LinearH`, `LinearV` | Row, Column | `onClick`, `onLongClick` |
| `ScrollH`, `ScrollV` | Scrolling row, Scrolling column | |
| `Button`, `TextView`, `ImageView` | Button, Text, Image | `onClick`, `onLongClick` |
| `EditText` | Text field | `onTextChanged` |
| `CheckBox`, `Switch` | Checkbox, Switch | `onCheckedChange` |
| `SeekBar` | Slider | `onProgressChanged` |
| `ListView`, `Spinner` | List, Dropdown | `onItemClicked`, `onItemSelected` |
| `VideoView` | Video | `onCompletion` |
| `WebView`, `ProgressBar`, `CalendarView` | Web page, Progress bar, Calendar | |

A widget's settings are plain fields: `width`/`height` (`Size::Match`, `Wrap` or `Px(n)`), padding, margin, gravity (`Gravity` across, `VGravity` up and down), weight, colours as ARGB `u32`, text and font, corner radius, shadow (`elevation`), border, picture, video options, and `input` (`InputKind`: text, password, email, number, decimal, phone, several lines). Layout follows Android's LinearLayout rules everywhere, including on iOS.

## Blocks

A `Block` is:

```rust
pub struct Block {
    pub id: u64,          // unique in the whole store (see new_id / bump_ids)
    pub op: String,       // which block: "setText", "repeat", "add", …
    pub args: Vec<Arg>,   // its slots, in the order of its spec
    pub sub1: Vec<Block>, // the inside of a C block (repeat, if…)
    pub sub2: Vec<Block>, // the "else" part of if/else
}

pub enum Arg {
    Text(String),         // something typed or picked in the slot
    Block(Box<Block>),    // a reporter block plugged into the slot
}
```

What each `op` looks like and where it goes in the palette is in `app/src/blocks.rs` (`DEFS`); what it does is in the engine and the code generators. In JSON (and in the MCP tools) a block reads like `{"op": "setText", "args": ["textview1", "Hello"]}`.

Block ids come from a global counter (`new_id`). `Store::load` calls `bump_ids` with the highest id found so new blocks never collide with saved ones. `Block::fresh` deep-copies a block with new ids (used for copy and paste).

## Events

An `Event` is `target` + `name` + `blocks`:

| `target` | `name` | Means |
|---|---|---|
| `""` | `onCreate` | the screen opens (every screen has it) |
| `""` | `onBackPressed` | the phone's back button or gesture |
| a widget id | one of its kind's `events()` | the widget was used |
| `"moreBlock"` | the block's name | a reusable block of your own ("More Block"), with `params` |

Components have no events of their own. Blocks that wait for something keep what happens next inside them: `timerAfter` and `timerEvery` run their inner stack (`sub1`) later, and a web request or Supabase block runs `sub1` when it succeeds and `sub2` when it fails.

`loose` keeps stacks lying unattached on the canvas, so the blocks editor can hold work in progress without running it.

## Components and variables

`Var { name, ty }` (number, text, true/false, map) and `ListVar` (lists of numbers, text or maps) are per screen. `Component { name, kind }` (`CompKind`: navigation, saved data, timer, pop-up, vibration, date and time, web request, sound player, camera) declares something blocks can use; it has no settings of its own.

## Files

`ProjFile` is one asset. The files themselves are stored outside the JSON, under `files_root()/<project id>/`. Their names are made into identifiers both phones accept (`file_base_name`: "My Cat!.png" → `my_cat`). `Project::add_file`, `rename_file` and `remove_file` keep the files and every block or widget that uses them in step (`file_uses`, `replace_file_uses`).

## Other helpers worth knowing

- `project_snapshot(p)` serialises a project without its modified time. The editor compares snapshots to know if anything changed (undo, "leave without saving?").
- `Project::rename_screen` renames a screen and every block that opens it; `screen_uses` lists those blocks.
- `bw_bytes(p)` makes a `.bw` project file: a zip (written and read by `archive.rs`) holding `project.json` and the project's files under `assets/`. `backup(p)` writes one to `~/Documents/Blockwise backups`.
- `read_project_file(path)` reads a `.bw` file, any zip with a `project.json` and its `assets` folder, or a bare `project.json` (files from an `assets` folder beside it) for **Import project**, and `Store::add_copy` adds it, or an example, under a new id and a free name.
- `leave_rule(auto_save, changed)` decides whether leaving a project saves, asks, or just leaves.
