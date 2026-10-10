# msime-pack-tool

`msime-pack` checks extension packs of every kind by exactly the rules the input method imports them by, so a pack author can check a pack before importing it or publishing it to the in-app community library.

```sh
cargo build -p msime-pack-tool --bin msime-pack
target/debug/msime-pack validate <pack folder or .zip>...
```

Every rule lives in `msime_client_core::plugins`; the tool calls `plugins::validate`, which stages the pack the way `plugins::import` does (copying a folder, or extracting a `.zip` under the same member, size and layout bounds) and runs the same checks, without a plugins directory and without installing anything. A change to a rule therefore needs no change here.

It prints one line per pack, in the order given:

```text
ok <id> <kind> <version>
error <path>: <code>: <reason>
```

`<code>` is the failure code the settings page decodes (`plugin_invalid`, `plugin_archive`, `plugin_unsupported_source`, `plugin_reserved`, `plugin_io`) and `<reason>` the broken rule in Chinese. The exit status is 0 when every pack is valid, 1 when any is not, and 2 for a command line it does not understand.

The author guide, with every field and limit, is [docs/plugins.md](../../docs/plugins.md); [docs/plugin-template](../../docs/plugin-template) is a minimal sound pack that validates.
