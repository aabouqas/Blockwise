# Contributing to Blockwise

Thanks for helping. Bug reports, ideas, docs and code are all welcome.

## Reporting a bug

Open an issue with:

- what you did, what you expected, and what happened instead;
- your system (macOS, Linux or Windows, and its version);
- a screenshot, or the project (Project ⋯ → *Back up* writes it to one file) if it helps.

For a security problem, follow [SECURITY.md](SECURITY.md) instead of opening a public issue.

## Changing the code

1. Read [docs/architecture.md](docs/architecture.md) for a map of the code, then the [developer guide](docs/developer/README.md) for the part you're changing. Its [recipes](docs/developer/recipes.md) list every file to touch when adding a block, a widget setting, a widget, an event or an MCP tool.
2. Build and test, from the `app/` folder:

   ```bash
   cd app
   cargo build --release
   cargo test --release
   ```

3. Keep each change focused, and add a test when you fix a bug or add a block. If a block behaves differently in the editor and on a phone, that's a bug; see `check.rs`.
4. If your change touches the UI, include before and after screenshots in the pull request (`Blockwise --screenshot out.png --open <scene>` helps).

### Style

- Match the style of the code around your change (it isn't run through `cargo fmt`). The build should have no warnings.
- Write UI text and comments in plain words for people who don't code: "Text field", not "EditText"; "Couldn't reach the server", not "HTTP error".
- No new dependency without a good reason. Every dependency must have a license compatible with Apache-2.0. List bundled assets in [THIRD_PARTY.md](THIRD_PARTY.md) with their license text in `licenses/`.
- Don't add images, fonts or code you don't have the right to share under Apache-2.0. For example, Apple's design resources may not be embedded in software.

## License of contributions

Blockwise is licensed under the [Apache License 2.0](LICENSE). Unless you say otherwise, anything you submit for inclusion is licensed under the same terms, as section 5 of the license says. You keep the copyright to your work.
