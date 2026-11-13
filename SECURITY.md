# Security

## Reporting a problem

Please don't open a public issue for a security problem. Report it privately through this repository's **Security** tab → **Report a vulnerability** (GitHub private vulnerability reporting).

Include what an attacker could do, how to reproduce it, and the version or commit you tested. You'll get a reply as soon as possible. Once a fix is out, you'll be credited unless you'd rather not be.

## Worth knowing

- The **MCP server** for AI assistants is off by default. When on, it listens on `127.0.0.1` only, so only programs on the same computer can connect. Any such program can read and change your projects while it's on.
- **Supabase**: link projects with the publishable key only. Blockwise refuses secret keys, because the key ships inside the app you build.
- The **Android setup** downloads tools only after you agree, and only from their official hosts: Adoptium (api.adoptium.net), Google (dl.google.com) and Gradle (services.gradle.org).
