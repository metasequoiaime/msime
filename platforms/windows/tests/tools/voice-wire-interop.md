# Portable voice controller interoperability

Run from the repository root:

```sh
bash platforms/windows/tests/tools/voice-wire-interop.sh
```

The Rust half lives at `crates/client-providers/tests/voice_wire_interop.rs`, so rustfmt and clippy reach it through the ordinary workspace commands and need no separate invocation here. Running `cargo test -p msime-client-providers` without `MSIME_VOICE_WIRE_PEER` reports that the peer is absent and passes; only the script above builds the peer (`platforms/windows/tests/voice/voice_wire_peer.cpp`) and makes the two tests do anything.

Requires a host C++17 compiler with AddressSanitizer/UndefinedBehaviorSanitizer and Rust. The runner compiles the real client-providers controller module and the real C++ dispatcher/codec against the contract headers in `shared/contracts`. A subprocess exchanges exact v2 messages inside bounded test-only length prefixes. Replies have a two-second receive deadline; teardown kills and reaps the peer.

Synthetic cases cover recording level, stop-once/poll progression, Unicode review output, mismatched request/session IDs, cancellation, failure and peer disconnect. The three Rust controller unit tests run alongside the two interoperability tests. No microphone, credentials, input injection or clipboard is used. This exercises the wire format between the two halves; named-pipe authentication, the Windows listener, focus transitions, device capture and native Tauri events are covered by `windows-voice-controller-connection`, `windows-voice-controller-listener`, `windows-voice-controller-dispatch` and `windows-voice-capture-selection`.
