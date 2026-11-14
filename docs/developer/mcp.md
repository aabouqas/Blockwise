# The MCP server

Blockwise lets AI assistants (Claude Code, Claude Desktop, any [MCP](https://modelcontextprotocol.io) client) build apps in the open window. Everything is in `app/src/mcp.rs`, plus `sync_mcp` and `mcp_call` in `app.rs`. For using it, see [AI assistants](../ai-assistants.md).

## Design in one paragraph

The server runs **inside the app**, on `127.0.0.1` only, while Settings › AI assistants is on. It speaks MCP's Streamable HTTP transport with plain JSON replies, no event streams. **Every tool call runs on the UI thread** against the same `Store` the window draws. So a change shows up live, lands in undo like any edit (undo is snapshots, see [the editor](editor.md#undo-and-redo)), and can never race the editor's own saving.

## The path of a tool call

```
client ──HTTP POST /mcp──▶ mcp-conn thread
                             serve → handle_post → answer (JSON-RPC)
                             tools/call → call_ui ──Call{tool,args,reply}──▶ channel
                                                                              │
UI thread, next frame: App::update → Server::poll → App::mcp_call ◀───────────┘
                         ├─ open_project, preview_screenshot,
                         │  run_on_device, device_status   (need the window)
                         └─ everything else → mcp::store_tool(&mut store, …)
                                                 └─ project_tool(project, …)
                       reply.send(Reply::Json | Image | Error) ──▶ back to HTTP
```

- `Server::start` binds the port and spawns an accept thread, then one thread per connection. `Server` drops cleanly: it sets a flag and wakes the accept loop.
- `call_ui` sends a `Call` with its own reply channel, calls `ctx.request_repaint()` so the UI thread wakes even when idle, and waits up to 90 seconds.
- `App::mcp_call` handles the tools that need the window itself. `preview_screenshot` opens the project, waits a few frames and replies with the phone cropped from a real window screenshot (`mcp_shot_step`, `phone_png`). The rest go to `store_tool`, which works on a `&mut Store` with no UI. That keeps most tools easy to test.
- `store_tool` updates the project's modified time only if the tool really changed something (snapshot before and after).

## Safety

- It listens on `127.0.0.1` only. `origin_ok` refuses requests from web pages whose `Origin` isn't localhost, so a website can't drive Blockwise from the person's browser.
- The server is off by default and runs only while Blockwise is open.
- `open_project` refuses to switch away from a project with unsaved changes.
- `link_supabase` refuses secret keys (`SupabaseLink::problem`); only publishable or anon keys can go into an app.
- Request bodies are capped (`MAX_BODY`).

## Tools

`TOOLS` lists every tool as `(name, title, description, input schema, required fields, read_only)`. `tools()` turns that into the `tools/list` answer. The descriptions are what the assistant reads, so keep them precise and plain. `INSTRUCTIONS` is the server-level guide sent at `initialize`.

Widgets are read and written as JSON with friendly names (`widget_json`, `apply_props`, `PROPS`): `"width": "match"`, `"background": "#FF5722"`, `"input": "password"`. Blocks are the same JSON the project file uses (`{"op": …, "args": […], "do": […], "else": […]}`), checked against `blocks::DEFS`. `list_blocks` explains every block's slots and drop-down choices to the assistant.

## The stdio bridge

Some clients only start servers as a child process over stdio. `Blockwise --mcp [--port N]` runs `bridge`: it reads JSON-RPC lines from stdin, POSTs each to the running app, and prints the replies. If the app isn't open, it answers with an error saying how to turn the server on.

## Adding a tool

See [Recipes → Add an MCP tool](recipes.md#add-an-mcp-tool).
