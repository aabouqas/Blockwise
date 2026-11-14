# AI assistants (MCP)

Blockwise can let an AI assistant work on your projects while you watch. It speaks the [Model Context Protocol](https://modelcontextprotocol.io) (MCP), so any MCP client can connect, including Claude Code and Claude Desktop. Everything the assistant changes appears live in the open window, and you can undo it with ⌘Z.

## Turning it on

1. Open **Settings** (⌘,) and find **Let AI assistants build apps**.
2. Tick **Turn on the MCP server**. The status shows the address it listens at, `http://127.0.0.1:7311/mcp` by default. Change the port if 7311 is taken.
3. Copy the snippet for your assistant from the same panel:

   **Claude Code**

   ```bash
   claude mcp add --transport http blockwise http://127.0.0.1:7311/mcp
   ```

   **Claude Desktop** (add to `claude_desktop_config.json`; Settings shows the exact path to your Blockwise):

   ```json
   {
     "mcpServers": {
       "blockwise": {
         "command": "/path/to/Blockwise",
         "args": ["--mcp", "--port", "7311"]
       }
     }
   }
   ```

   `Blockwise --mcp` relays a client that only speaks stdio to the running app.

The server is off by default, it listens only on this computer, and it works only while Blockwise is open.

## What an assistant can do

| Tool | Does |
|---|---|
| `list_projects`, `create_project`, `open_project` | Find, make and open projects |
| `get_screen` | Read a screen: its widget tree, variables, components and events |
| `add_screen` | Add a screen, blank or from a template |
| `update_screen` | Set a screen's title, its app bar (shown or hidden, colours, back button, icon buttons) and whether it opens as a bottom sheet |
| `add_widget`, `update_widget`, `move_widget`, `delete_widget` | Build the layout (`add_widget` can add a whole tree at once) |
| `add_variable`, `add_component` | Add what the blocks need |
| `list_blocks`, `set_event_blocks`, `set_more_block` | Program events and reusable blocks |
| `add_file`, `list_files`, `rename_file`, `delete_file` | Manage assets |
| `link_supabase` | Link a Supabase project (publishable keys only) |
| `preview_screenshot` | Look at the screen as the preview draws it |
| `get_code` | Read the generated Kotlin, Swift or layout XML |
| `run_on_device`, `device_status` | Build and run on the emulator or simulator, and see how it went |

Blocks are JSON, for example:

```json
{"op": "setText", "args": ["textview1", "Hello"]}
```

An argument can itself be a block. C-shaped blocks take `"do": [...]`, and if/else also takes `"else": [...]`.

## Example requests

- "Make a tip calculator: an amount field, a slider for the percentage, and a text showing the tip."
- "Add a login screen that signs in with Supabase and opens the main screen."
- "Run it on the Android emulator and tell me if anything goes wrong."
