# Shared voice provider adaptation

`msime-voice-providers` contains the provider defaults, HTTP recognition and polishing adapter extracted unchanged from `platforms/windows/src/VoiceProviders.h` at the shared client repository, commit `f78cf0fdf4f58d3e9405faec3158ed216b17ec78`. It encodes the upload as 16-bit WAV itself, builds the multipart body with libcurl's MIME API, parses the JSON answers with nlohmann/json, and throws `msime::voice::VoiceError` (`VoiceError.h`). It has no desktop UI, Windows API, host-process, or microphone dependency.

Native hosts include `VoiceProviders.h` and use `msime::voice`. The historical `msime::windows` symbols remain for compatibility, with a forwarding header at the old Windows include path. There is only one implementation. No TSF/Server protocol or process ownership changes are involved.

Hosts must supply bounded mono 16 kHz PCM, configuration, cancellation and a worker queue, then validate session/focus before applying results. This module does not capture audio or implement Doubao WebSocket transport; capture, the Doubao session and the wave overlay belong to each platform host (`platforms/macos/src/voice/`, `platforms/windows/src/voice/`, `crates/host-windows/src/voice_controller.rs`).

Standalone local validation:

```sh
cmake -S shared/voice -B build/shared-voice -DBUILD_TESTING=ON
cmake --build build/shared-voice
ctest --test-dir build/shared-voice --output-on-failure
```

The synthetic routing test exercises the platform-neutral entry points and pre-cancelled requests without contacting cloud services. `tests/transport.py` runs the real curl adapter against a loopback fixture server and checks the multipart upload (model, language and WAV parts) as well as rejection, redirect, malformed, oversized and retry answers.

`DoubaoAuth.h` is a thin C++ adapter for `client-core::doubao_auth` through `msime_client_doubao_auth_headers`. It requires the host-api include path and library. Both the Rust credential probe and native Windows recording use this same authentication policy: explicit `api_key` ignores stale App IDs, explicit `legacy` requires an App ID, and missing historical modes infer from a usable App ID. Returned header text contains credentials and must never be logged. Pass an absolute `MSIME_HOST_LIBRARY` when configuring this project to enable the synthetic C++/Rust ABI test `shared-doubao-auth`.

## On-device recognition (LocalAsr)

`LocalAsr.h` (target `msime-voice-local-asr`) recognizes on this machine through the sherpa-onnx C API. The runtime is pinned per platform by `resources/voice-runtime.lock.json` and fetched with `scripts/fetch_voice_runtime.py --platform <platform>` into `target/voice-runtime/<platform>/`. It is not linked: the library is loaded with `dlopen`/`LoadLibrary` on first use, so a package without it still starts and `sherpa_runtime_available()` answers false. The library is looked for in `set_sherpa_library_path()`, then `MSIME_SHERPA_ONNX_LIBRARY`, then beside the executable, then in a macOS bundle's `Contents/Frameworks` (other Unix hosts: `../lib/msime/` and `../lib/` relative to the executable), then by bare name. The first load attempt decides for the life of the process. Only the C header is vendored, in `third_party/sherpa-onnx/` with its Apache-2.0 license.

A model is a directory holding the files of one entry of `resources/local-asr-models.json` plus `msime-model.json`, a verbatim copy of that entry. The installer (`client-core::voice::local_models`, exposed as `msime_client_voice_local_model_install`) writes the manifest last, so a directory without it is never treated as a model. Nothing in this module downloads anything.

- Streaming models (`online_transducer`) decode as audio arrives. Whole-utterance models (`offline_sense_voice`, `offline_funasr_nano`) are fed through Silero VAD and decode each finished speech segment. Both report partial text before `finish()`.
- `hotwords` are used natively when the manifest says `"hotwords": "native"` and the model has what it needs (the transducer needs `bpe_vocab` and a `modeling_unit`). For `"pinyin"` models the recognizer ignores them and the host corrects the final text with `msime_client_voice_hotword_correct`. Hosts get the words from `msime_client_voice_hotwords`, which reads the user's own pinyin dictionary entries.
- A loaded model is kept for the next dictation. Hosts call `release_idle_local_models()` from a timer, since a model holds hundreds of megabytes to over a gigabyte.
- `recognize_local_asr()` in `VoiceProviders.h` recognizes an installed model directory here and rejects any other path. Windows calls it in-process in `msime-client-server`.

### The msime-voice-local helper

Input method processes (macOS IMK, Linux) should not hold a speech model: they are loaded into every app that takes text, and a crash there takes typing down with it. They spawn `msime-voice-local` instead and talk to it over stdin/stdout, one UTF-8 JSON object per line in each direction.

```
msime-voice-local [--runtime <library>] [--idle-exit <seconds>]
msime-voice-local [--runtime <library>] --model <dir> --wav <file> [--language <tag>] [--hotword <word>]...
```

The second form transcribes a 16 kHz mono 16-bit PCM WAV file, prints the text and exits; it is for tests and for checking an installed model by hand. `--idle-exit` takes whole non-negative seconds and defaults to 600; 0 disables it.

On start the helper writes `{"type":"hello","version":1,"available":<bool>,"error":"<why the runtime did not load, or empty>"}`. The runtime is loaded to answer it, not the model.

| Request | Reply |
| --- | --- |
| `{"op":"start","id":X,"model":"<model dir>","language":"zh-CN","hotwords":["..."],"threads":0}` | `{"type":"started","id":X}`. Loads the model if it is not loaded. A new `start` replaces any open session without a reply for the old one. `language`, `hotwords` and `threads` are optional; `threads` 0 picks from the hardware. |
| `{"op":"audio","pcm16":"<base64>"}` | Zero or more `{"type":"partial","id":X,"text":"..."}`. The payload is little-endian signed 16-bit PCM, 16 kHz mono. `text` is the whole transcript so far, sent only when it changed. Ignored when no session is open. |
| `{"op":"finish"}` | `{"type":"final","id":X,"text":"..."}`, and the session is closed. Ignored when no session is open. |
| `{"op":"cancel"}` | `{"type":"cancelled","id":X}`. Takes effect in the middle of a decode, ahead of queued requests. No reply when no session is open. |
| `{"op":"release"}` | No reply. Drops every loaded model not in use. |
| `{"op":"ping","id":X}` | `{"type":"pong","id":X,"available":<bool>}`. |

Any failure produces `{"type":"error","id":X,"message":"..."}` and closes the session; `id` is the request's for a failed `start` and the open session's otherwise. A line that is not JSON gets an error without `id`, and an unknown `op` gets an error too. A failure caused by `cancel` is reported as `cancelled`, so a cancelled session always ends with exactly one `cancelled`.
Requests are limited to 1 MiB per line, and the helper retains at most 8 MiB of serialized requests waiting for the worker. An oversized line gets `{"type":"error","message":"request too large"}` and is discarded through its newline before the next request is read. If the queue budget is exceeded, pending requests are discarded and the active session ends with `{"type":"error","id":<session>,"message":"request queue full"}`; this keeps the input writer asynchronous while bounding helper memory.

Requests are handled in order on one worker thread. A loaded model is released after 120 seconds without a session (or `--idle-exit`, if shorter and not 0), and the process exits on stdin EOF or after `--idle-exit` seconds without a request while no session is open. Hosts should respawn it on demand rather than keep it alive.

With a fetched runtime and an installed model, `tests/local_helper.py` drives this protocol end to end:

```sh
python3 scripts/fetch_voice_runtime.py --platform macos
cmake -S shared/voice -B build/shared-voice -DBUILD_TESTING=ON \
  -DMSIME_LOCAL_ASR_TEST_RUNTIME="$PWD/target/voice-runtime/macos/libsherpa-onnx-c-api.dylib" \
  -DMSIME_LOCAL_ASR_TEST_MODEL=<installed model dir>
cmake --build build/shared-voice
ctest --test-dir build/shared-voice -R shared-voice-local --output-on-failure
```
