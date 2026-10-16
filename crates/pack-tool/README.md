# msime-pack-tool

`msime-pack` checks extension packs of every kind by exactly the rules the input method imports them by, so a pack author can check a pack before importing it or publishing it to the in-app community library.

```sh
cargo build -p msime-pack-tool --bin msime-pack
target/debug/msime-pack validate [--builtin] <pack folder or .zip>...
```

Every rule lives in `msime_client_services::plugins`; the tool calls `plugins::validate`, which stages the pack the way `plugins::import` does (copying a folder, or extracting a `.zip` under the same member, size and layout bounds) and runs the same checks, without a plugins directory and without installing anything. A change to a rule therefore needs no change here.

It prints one line per pack, in the order given:

```text
ok <id> <kind> <version>
error <path>: <code>: <reason>
```

`<code>` is the failure code the settings page decodes (`plugin_invalid`, `plugin_archive`, `plugin_unsupported_source`, `plugin_reserved`, `plugin_io`) and `<reason>` the broken rule in Chinese. The exit status is 0 when every pack is valid, 1 when any is not, and 2 for a command line it does not understand.

## Running a script plugin (desktop only)

```sh
target/debug/msime-pack run <pack folder or .zip> --events <fixture.jsonl> [--grant all|<capability>,...] [--builtin]
```

`run` checks the pack by the `validate` rules, then runs the script plugin against the events in the fixture on the same sandboxed runtime the input method uses (`msime-script-runtime`), and prints one JSON line per event: `seq`, `event`, `status`, `fuel`, `actions` and `log`, plus `init_fuel` when the event instantiated the plugin or tried to (on `init:*` and `deferred` lines it is the fuel the failed or cut-off attempt used), and on `command` lines `rows`, the `[text, note]` pairs the plugin gave (empty when it did not return normally). A `command` event is written `{"event":"command","trigger":"uuid","argument":"","random":"<128 hex digits>"}`: the trigger must be one the pack declares in `[[script.commands]]`, the argument is lowercase letters (empty when left out) and `random` is the event's 64 random bytes (all zero when left out). The exit status is 0 when no event failed, 1 when any did or the pack is invalid, and 2 for a command line, `--grant` or fixture it does not understand. The fixture format, the status values and where `run` differs from the input method are in the 「用 msime-pack run 试运行」 part of [docs/plugins.md](../../docs/plugins.md).

## The content hash of a script pack (desktop only)

```sh
target/debug/msime-pack hash [--builtin] <pack folder or .zip>
```

`hash` checks a script pack by the `validate` rules and prints its content hash, the 64 lowercase hex digits a grant is bound to; a pack it refuses gets the same `error` line as `validate`. `scripts/build-official-plugins.sh` compares it with the hash `plugins/official-packs.json` locks for each official script plugin.

## Official packs

The official packs the desktop installers ship (`plugins/official-packs.json`: the uuid script plugin and the latex-symbols phrase table) use ids the input method reserves, so `validate`, `run` and `hash` refuse them with `plugin_reserved` as an import would. `--builtin` lifts that one check for the repository's own build scripts; every other rule still applies. Pack authors never need it.

The author guide, with every field and limit, is [docs/plugins.md](../../docs/plugins.md); [docs/plugin-template](../../docs/plugin-template) is a minimal sound pack that validates, and [docs/script-plugin-template](../../docs/script-plugin-template) a script plugin with the C header and Rust binding.
