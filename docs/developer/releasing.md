# Releasing

A release is a version tag. Pushing one makes GitHub build the downloads for macOS, Windows and Linux and put them on a draft release. You check the draft, then publish it.

## Making a release

1. Set the new version in `app/Cargo.toml` (`version = "0.2.0"`), run `cargo build` once so `Cargo.lock` follows, and commit both files.
2. Tag that commit and push the tag. The tag is the version with a `v` in front:

   ```bash
   git tag v0.2.0
   git push origin main v0.2.0
   ```

3. Wait for **Release** to finish in the Actions tab (about 15 minutes). It stops at once if the tag and `Cargo.toml` don't agree.
4. Open **Releases** on GitHub. The draft has the three downloads and notes written from the pull requests and commits since the last release. Edit the notes if needed, download the files and try them, then press **Publish release**.

To redo a release that went wrong, delete the draft and the tag (`git push origin :v0.2.0` and `git tag -d v0.2.0`), fix the problem, and tag again.

## What gets built

`.github/workflows/release.yml` tests and packages on each system with `app/package.sh`:

| Download | Contents |
|---|---|
| `Blockwise-<version>-macOS.dmg` | `Blockwise.app`, one app for Apple silicon and Intel (macOS 11 or later), and a link to drag it into Applications |
| `Blockwise-<version>-Windows-x86_64.zip` | `Blockwise.exe` |
| `Blockwise-<version>-Linux-x86_64.tar.gz` | the `Blockwise` program, a `.desktop` file and its icon. Built on Ubuntu 22.04, so it runs on that and newer. |

Each one also carries `LICENSE`, `NOTICE`, `THIRD_PARTY.md` and `licenses/`. The Apache License and the bundled fonts' and icons' licenses require them.

You can make the same packages on your own computer (`dist/` in `app/`):

```bash
cd app
./package.sh macos      # needs: rustup target add x86_64-apple-darwin
./package.sh linux
./package.sh windows    # in Git Bash
```

## Signing

The downloads are not signed with an Apple Developer ID or a Windows code-signing certificate, so the first time someone opens Blockwise:

- **macOS** says it can't check the app for malicious software. Open **System Settings › Privacy & Security** and press **Open Anyway** below the message. Or run `xattr -dr com.apple.quarantine /Applications/Blockwise.app` in Terminal.
- **Windows** SmartScreen says it protected the PC. Press **More info**, then **Run anyway**.

Both are one-time. Signing removes them, but it needs a paid Apple Developer account (for signing and notarising) and a Windows code-signing certificate. Once those exist, add the signing steps to `package.sh`, with the keys kept in the repository's Actions secrets.

## Windows console

Release builds use the Windows GUI subsystem (the `windows_subsystem` line at the top of `main.rs`), so no console window opens next to the app. `Blockwise.exe --mcp` still works, because MCP clients start it with their own pipes. Debug builds keep the console for log output.
