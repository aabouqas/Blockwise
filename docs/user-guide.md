# Blockwise user guide

This guide walks through making an app, from a new project to a phone. Shortcuts use ⌘ on macOS; use Ctrl on Linux and Windows.

- [Projects](#projects)
- [Screens](#screens)
- [The designer](#the-designer)
- [Events and blocks](#events-and-blocks)
- [Components](#components)
- [Assets](#assets)
- [Trying your app](#trying-your-app)
- [Debugging](#debugging)
- [Running on a phone](#running-on-a-phone)
- [Getting the code](#getting-the-code)
- [Supabase: sign-in and online data](#supabase-sign-in-and-online-data)
- [Settings](#settings)
- [Shortcuts](#shortcuts)

## Projects

The **Projects** page lists your apps. Click **New project** (⌘N) and give it:

- a **project name** (what Blockwise calls it),
- an **app name** (what the phone shows under the icon),
- an **app id** such as `com.you.counter` (the package name; it must be unique on a phone),
- an icon and a version.

Each project's **⋯** menu has *Project settings*, *Back up* (one file holding the whole project, saved in `Documents/Blockwise backups`), *Export source* and *Duplicate*. Search with ⌘F, and switch between grid and list.

The **Examples** page, in the left column, has finished apps to learn from: *Component Tour* (every native widget, app bar buttons, a pop-up and a bottom sheet) and *Brain Quiz* (three screens, a timer, a JSON data file and a saved best score). **Open a copy** adds one to your projects, with its files, and opens it; change it as you like. On the first start Blockwise adds a copy of each. See them running in [examples](../examples/README.md).

Projects are saved in your user data folder (`~/Library/Application Support/Blockwise` on macOS). With **Automatically save** off (the default), leaving a changed project asks whether to keep the changes. ⌘S saves at any time, and ⌘Z / ⇧⌘Z undo and redo.

## Screens

An app has one or more screens. The picker at the top left shows the current one:

- **+ Screen** adds a screen, blank or from a template: *Log in*, *Sign up*, *Settings*, *List*, or *Log in with Supabase*.
- The **pencil** beside a screen renames it. Blocks that open that screen follow the new name.
- The **x** beside a screen deletes it, after asking. The first screen, where the app starts, can't be deleted.

### The app bar

The bar at the top of a screen, with its title. Click an empty part of the phone (so no widget is selected) and the inspector shows the screen's settings:

- **Title**: what the bar says. Left empty, it shows the app's name. The *set screen title* block changes it while the app runs.
- **App bar**: untick **Show** for a screen without a bar, such as a game or a splash screen. It then starts right under the status bar. If the screen's outer layout fills the screen with a colour, the status bar takes that colour too, so a full-colour page reaches the top of the phone.
- **Bar colour** and **Title colour**: *default* is each phone's own look: the theme colour with a white title on Android, the system bar on iPhone. A title colour left as default is white or black, whichever reads on the bar. On Android the status bar takes a darker shade of the bar.
- **Back button**: a screen opened from another has a back arrow in its bar. Untick it to leave the arrow out (the phone's own back still works).

On iPhone the back button is the system's, so it keeps its own colour.

**App bar buttons** are icons on the right of the bar, such as search, add or share. Press **+ Add button** under the screen's settings, pick an icon and give it a label (screen readers read it; Android shows it on a long press). Each button has an id, like `searchButton`. In the **Logic** tab, **Add event** lists *When searchButton is clicked*: put its blocks there. On Android the icons take the title's colour; on iPhone they sit together in the bar's glass button.

### Bottom sheets

Any screen but the first can open as a bottom sheet: a panel that slides up over the screen that opens it, which stays dimmed above. In the screen's settings set **Opens as** to *Bottom sheet, half* (half the phone's height) or *Bottom sheet, tall* (nearly all of it). Design it like any screen; a sheet has no app bar.

Open it with the *open screen* block. *Close this screen* closes it, and so does a tap above it, the phone's back, or (on iPhone) pulling it down. Use it for a choice, a short form or details about an item; to send back what was picked, save it with a *saved data* component and read it when the screen below needs it.

## The designer

The **Design** tab (⌘1) has three parts:

- **Insert** (left): layouts and widgets. Drag one onto the phone, or click it to add it to the selected layout. The **Outline** below shows every widget as a tree.
- **The phone** (middle): your screen as it will look. Switch between the **Android** and **iOS** look at the top right.
- **The inspector** (right): the selected widget's settings: id, size, padding, margin, gravity, weight, background, corners, shadow, border, text, font, colours and more.

Layouts:

- **Row** and **Column** place their children side by side or one under another. A child's **weight** shares out the spare room; the layout's **gravity** places its children.
- **Scrolling row** and **Scrolling column** do the same, and scroll when the content doesn't fit.

Widgets: Button, Text, Text field, Image, Web page, Progress bar, List, Dropdown, Checkbox, Switch, Slider, Calendar and Video.

**Text fields** have an **Input** setting for what they take:

| Input | On the phone |
|---|---|
| Text | Plain text |
| Password | Dots, with an eye to show or hide what's typed |
| Email | The email keyboard |
| Number | Whole numbers |
| Decimal number | Numbers with a decimal point |
| Phone number | The phone keypad |
| Several lines | Grows as you type |

⌘D duplicates the selected widget, and Delete removes it.

## Events and blocks

The **Events** tab (⌘2) lists what your app reacts to: the screen opening, buttons being clicked, text changing, and so on.

1. Click **Add event**. Type to search (for example "button click" or "text change"), then press Enter to add the first match, or click the one you want.
2. Click an event to open its blocks.
3. Drag blocks from the palette on the right into the event. Search the palette at the top.

Block categories:

| Category | For |
|---|---|
| Variable | Numbers, text, true/false values and maps that the screen remembers |
| List | Lists of numbers, text or maps |
| Control | If / else, repeat, wait, and opening other screens |
| Operator | Comparing, joining text, and/or/not |
| Math | Arithmetic, rounding and random numbers |
| Saved data | Values kept after the app closes |
| View | Reading and changing widgets: text, colours, visibility, lists… |
| Component | Timers, pop-ups, sounds, the camera, web requests… |
| More Block | Your own blocks, with inputs, to reuse logic |
| Supabase | Signing people in and reading/writing tables |

In the block area:

- Drag to select.
- Shift-click adds to the selection.
- Alt-drag copies.
- Scroll or Space-drag moves around.
- Right-click opens a block's menu, which includes setting a breakpoint.

## Components

The **Components** tab (⌘3) adds the parts that blocks need:

- **Navigation**: opens other screens and passes values.
- **Saved data**: keeps values on the phone.
- **Timer**: runs blocks later or repeatedly.
- **Pop-up**: shows a dialog.
- **Vibration**.
- **Date & time**.
- **Web request**: calls web services.
- **Sound player**: plays sounds.
- **Camera**: takes photos and records videos.

## Assets

The **Assets** tab (⌘4) holds your app's files: pictures, sounds, videos, fonts and data files. Add files there, then use them in widgets (an Image's picture, a Video's clip, a Text's font) and in blocks. Click a file to view or play it.

## Trying your app

Press **▶ Play** above the phone in the designer. The phone becomes the app: tap buttons, type, scroll, play media. Widgets can't be dragged while it plays. Press **■ Stop** to go back to designing.

## Debugging

Click **Debug** in the top bar. The app starts on the designer's phone with a debugger docked at the bottom of the window:

- **Debugger**: what ran, step by step. When a breakpoint is hit, use *Step*, *Continue* or *Run to end*. Tick *Pause at the start of every event* to stop everywhere.
- **App state**: the current values of variables and lists.
- **Device log**: the log of an app running on the emulator or simulator.

To set a breakpoint, right-click a block. The debugger opens by itself when one is hit. **Debug** also opens and closes the dock while something is running.

## Running on a phone

Click **Run** (⌘R) and pick a device:

- **Android emulator.** The first time, Blockwise lists what is missing (Java 21, the Android SDK, Gradle and an emulator image, about 2.4 GB) and asks before downloading anything. It uses Android Studio's tools if they are already installed.
- **iPhone simulator** (macOS with Xcode). Blockwise compiles the SwiftUI app and opens it in the simulator.

Run reads *Building…* while it works. If an Android app crashes, it reopens on a screen showing the error, the same way it does in Play.

## Getting the code

- **Code** (⌘J) shows the code of the current screen as you design: Kotlin, the XML layout, or Swift.
- **Build** (⌘B) makes an Android APK.
- **Export source** (⌘E) writes an Android Studio project (Kotlin + XML), an Xcode-ready SwiftUI project, or both, into your export folder.

The generated code is yours: use it, change it and ship it under any license (see [NOTICE](../NOTICE)).

## Supabase: sign-in and online data

1. Create a project at [supabase.com](https://supabase.com).
2. In the blocks palette, open the **Supabase** category and click **Link Supabase…**. Enter the project's URL and its **publishable** key. Never use the secret key: it would ship inside your app.
3. Add a screen from the *Log in with Supabase* template, or use the **Supabase** blocks directly.

Supabase blocks run their *do* part when a call succeeds and their *else* part when it fails. Inside them, *response* holds the answer (rows come as a JSON list) and *error message* a readable error.

## Settings

Open Settings with ⌘, to change:

- the theme and the size of the interface,
- the phone the preview shows,
- the code text size,
- whether deleting asks first,
- whether the last project reopens when Blockwise starts,
- automatic saving,
- the export folder,
- the [MCP server for AI assistants](ai-assistants.md).

## Shortcuts

| Keys | Does |
|---|---|
| ⌘N | New project |
| ⌘F | Search projects |
| ⌘1 – ⌘4 | Design, Events, Components, Assets |
| ⌘R | Run |
| ⌘B | Build |
| ⌘E | Export source |
| ⌘J | Show the code |
| ⌘S | Save |
| ⌘Z / ⇧⌘Z | Undo / redo |
| ⌘D | Duplicate the selection |
| Delete | Delete the selection |
| ⌘, | Settings |
| Esc | Close the open event, dialog or settings |
