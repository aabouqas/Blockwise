# The engine

`app/src/runtime.rs` runs a project's blocks inside Blockwise. Play, Debug and several MCP tools use it, so an app can be tried without a build. Its one rule: **behave like the generated Android app.** Numbers are `Double`s, text is formatted like Java, a mistake that would crash the phone crashes the preview, and an endless loop freezes it the way Android would.

## The `Runtime` value

`Runtime::new(project, now)` (or `starting_on` / `with_debug`) clones the project and opens its first screen. The runtime then holds the whole live state of the app:

| Field | What it is |
|---|---|
| `screen_idx`, `back_stack` | the screen showing and the ones under it |
| `widgets: HashMap<id, WState>` | each widget's live text, colours, visibility, list items, progress… (`initial_state` fills it from the `Widget`) |
| `vars`, `lists`, `maps`, `list_maps` | the blocks' variables, as `Val::Num`, `Val::Str` or `Val::Bool` |
| `prefs` | saved data, kept across screens like SharedPreferences |
| `toasts`, `dialog`, `title`, `vibrating_until` | what the pretend phone shows |
| `timers`, `web` | work that finishes later |
| `trace`, `breakpoints`, `pause`, `crash` | debugging and crashes (below) |

The editor draws `rt.screen()` with `rt.widgets` through `phone.rs`, and calls two things on it:

- **`fire(target, name, locals)`** when the person uses a widget. It finds the matching `Event` on the current screen and runs its blocks. `locals` carries event values such as `_position` (a list item), `_isChecked`, `_charSeq` (typed text) or `_progressValue`.
- **`tick(now)`** every frame. It runs timers that are due and the answer stacks of web requests that finished.

Opening a screen (`open_screen`) resets the widget states for that screen and fires its `onCreate`. The back button pops `back_stack` (or fires `onBackPressed` if the screen has it).

## Running blocks

```
fire → run_blocks → run_blocks_ctl → exec_stack → exec → exec_inner
                                                         └→ eval (reporters)
```

- `exec_stack` runs a list of blocks in order and stops early on a `Flow` other than `Normal` (`break`, stopping after a crash or a pause).
- `exec` does the work around every statement. It checks for a breakpoint, counts the step, writes a trace line, works out plugged-in reporter blocks left to right (like Java evaluates arguments), then calls `exec_inner`. After a variable block it records the new value in `values`. Debug builds on phones log the same values, which is what [the phone checks](testing.md#the-phone-checks) compare.
- `exec_inner` is one big `match` on `b.op`: one arm per statement block.
- `eval` is the same for reporter blocks (numbers, text, true/false).

**Endless loops.** `step` looks at the clock every 4096 blocks. An event running longer than `EVENT_SECONDS` (5 s, as Android's "not responding"; 0.5 s in tests) is stopped and logged.

**Crashes.** A block that would throw on a phone (a list index out of range, a number that isn't one…) calls `crash(exception, detail, problem)`. The runtime stops everything (timers, web requests, dialogs) and, after `CRASH_RESTART_S`, the phone shows the crash screen with `CRASH_TITLE` and the error. This matches the crash screen generated Android apps show.

**Waiting.** Blocks never block the UI thread. `timerAfter` and `timerEvery` add a `TimerJob` with their inner stack. Web and Supabase requests run `web_fetch` on a thread (`web_job`) and store the result in a shared slot. `tick` later runs the success (`sub1`) or failure (`sub2`) stack with `_response`, `_code` and `_ok` set.

## The debugger

Breakpoints are block ids (set with a right-click in the blocks editor). With `step_mode` on, the runtime pauses before the first block of every event.

Pausing works by **snapshot and replay**, not by suspending Rust code:

1. When debugging, `run_blocks_ctl` clones the whole runtime *before* the event runs (`pre`).
2. `exec` numbers every statement it reaches (`ctl.stmt`). When it reaches a breakpoint (or the statement number it was told to stop at), it records the spot in `ctl.hit` and returns `Flow::Stop`.
3. `run_blocks_ctl` sees the hit and stores a `Pause`: the snapshot, the event's blocks, its locals and the statement number.
4. **Step** restores the snapshot and runs the same event again, stopping one statement further (`debug_step`). **Continue** replays it and stops at the next breakpoint after the current spot (`debug_continue`). **Run to end** replays without stopping (`debug_finish`). See `replay`.

While paused, `tick` freezes timers and toasts, and a replay shifts their due times by the time spent paused. Because the event re-runs from a snapshot of the whole state, every replay gives exactly the same result. Web requests inside a paused event are the one thing that is fetched again.

The debugger dock reads `trace` (what ran, with depth for indentation), `vars` / `lists` for the app state view, and `pause` for where it stopped.

## Adding behaviour

A new statement block needs an arm in `exec_inner`; a new reporter needs one in `eval`. Keep these rules:

- Read slots with `self.txt(b, i)`, `self.num(b, i)` and `self.boolean(b, i)`, and names (variables, widgets, components) with `Self::name(b, i)`.
- Format numbers with `fmt_num` / `fixed_text` so they read like the phones (`3`, not `3.0`; Java's rounding).
- If the phone would crash, call `self.crash(…)` with the same exception name Kotlin would throw.
- Then make the [code generators](code-generation.md) do the same thing, and add the block to a [phone check](testing.md#the-phone-checks).

[Recipes → Add a block](recipes.md#add-a-block) has the full checklist.
