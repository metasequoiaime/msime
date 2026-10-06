# Shared text-tool input sessions

The native IMK emoji, handwriting and cloud clipboard routes launch their shared Tauri panels in a new application instance. Each captures the exact weak IMK client and generation before launch. Shared settings launched without this context cannot insert text through this bridge. A session is limited to the launching route: another tool window in that process cannot submit or cancel it. Emoji/handwriting launch-failure fallbacks retain the same captured target; cloud clipboard falls back to its copy-only native window. The handwriting fallback closes only after an accepted selection, and standalone native handwriting copies instead of guessing a client.

## Packaged handwriting

The macOS release package no longer carries the handwriting model. The settings app downloads the pinned model (`resources/handwriting-model.lock.json`) the first time the handwriting panel opens, into `resource-packs/handwriting` under its data directory (`~/Library/Application Support/app.msime.macos` by default), with the Tegaki model license beside it; the Zinnia license still ships under `Contents/Resources/handwriting`. Lookup prefers the downloaded copy and falls back to a model under `Contents/Resources/handwriting` (a development bundle, or a package built before the model was dropped), which is found relative to the running executable so moving the application does not break recognition. Explicit host options and provider/model overrides retain precedence.

The engine's zinnia port (`crates/engine/src/handwriting`) performs single-character ordered-stroke recognition; host-api splits a written line into character cells first. This is not sentence segmentation or image OCR. An exploratory Apple Vision adapter executed successfully but returned no candidates for the tested isolated glyphs; it was removed rather than treating a successful API call as handwriting parity. The shared panel uses the packaged engine implementation without needing a separate recognizer service. Recognition and composition algorithms remain out of the platform host and UI.

`pnpm --filter @msime/desktop tauri build --debug --bundles app --no-sign` produces an unsigned local bundle for development. Release signing and installation are described in `README.md`.

## Transport

The session environment contains protocol version, a private temporary Unix socket path, the native host PID, and the original application PID and launch time. It never reaches JavaScript and contains no composed text or credential. A 0700 directory and 0600 socket restrict access; both endpoints additionally verify kernel-provided UID and PID. The native endpoint authorizes only the launched application and checks that it remains alive.

A request is a four-byte big-endian UTF-8 byte length followed by at most 4096 bytes and write EOF for candidate sessions. Cloud clipboard sessions explicitly declare `clipboard: true` in the native launch configuration and support 4000 UTF-16 units, at most 12000 UTF-8 bytes. CR, LF and tab are preserved; other Unicode Cc control characters are rejected. Format scalars such as emoji ZWJ remain valid, matching the shared Rust contract. Absent `clipboard` selects the candidate transport contract. The Tauri dispatcher requires both the clipboard route and this session type before accepting clipboard text; candidate panels use their stricter candidate validator. NUL, invalid UTF-8, trailing bytes, and truncated frames are rejected. Zero length cancels an unused session. A single response byte is 0 for confirmed native insertion and 1 for rejection. Transport failure after writing is an unknown outcome, never an automatic retry. A session allows one submission, expires after five minutes, and removes its socket and directory when stopped. Accepted Darwin sockets explicitly clear the inherited nonblocking flag so fragments wait for the configured receive timeout instead of failing immediately.

On selection, Tauri checks the target application's launch identity and refuses to take focus from a third application, restores the original application, hides the panel, and submits on a blocking worker. IMK retains ownership of exact-client validation and insertion. Its pending text expires at the transport's monotonic deadline (at most two seconds), and only an actual insert call acknowledges success. Close and submit are serialized; cancellation does not insert text. Neither component uses accessibility text injection or requests permissions.

## Local verification

- `cargo test -p msime-host-macos` covers real Unix transport framing, wrong-peer rejection, lost acknowledgements, input bounds, and invalid configuration.
- `cargo test -p msime-desktop --lib` covers shared tool startup routing, route isolation, mutually exclusive close/submit, and real Engine recognition of synthetic Chinese strokes from a relocated bundle-shaped resource directory.
- `desktop-input-session-test` covers native framing, cancellation, refusal, malformed requests, duplicate completion, permissions, and cleanup.
- Build `cargo build -p msime-host-macos --example panel_session_probe`, configure CMake with `MSIME_PANEL_SESSION_PROBE` pointing to that binary, and run `desktop-input-interop` for a real Rust child process/native listener exchange.
- `desktop-settings-launcher-test` checks route/environment propagation and the main-thread launch authorization callback. `tool-text-return-test` checks exact-client, generation, and expiration guards.
- Build `MSIMEClientInputMethod` to compile the Objective-C IMK integration.

These tests use synthetic input and do not activate editors. The cloud interop test combines native account RPC and one-shot input sockets in the same Rust child process, then takes the full multiline text through the captured-client generation guard.

Reference baseline for the clipboard contract: MSIME-Windows default branch `develop` at `cb534a97fd19bc9656645a7baa4ee019487279a8`.
