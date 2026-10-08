# Agent Note: Android WebSocket Connection token validation

Status: implemented

## Problem

The Android WebSocket handshake accepted a `Connection` response such as `Connection: notupgrade` because it searched for `upgrade` as a substring. RFC 6455 requires the `Upgrade` connection token in the comma-separated field value.

## Decision

Parse `Connection` as a comma-separated token list, trim each token, and compare it case-insensitively. Accumulate valid tokens across repeated `Connection` fields.

## Alternatives considered

- **Keep substring matching**: accepts malformed handshake responses and can confuse HTTP and WebSocket protocol state.
- **Require the whole field to equal `Upgrade`**: rejects valid responses that list `keep-alive, Upgrade`.
- **Add a dependency for HTTP parsing**: unnecessary for this bounded token check and would expand the Android host footprint.

## Verification

Added JVM smoke cases for rejecting `notupgrade` and accepting `keep-alive, UpGrAdE`. `platforms/android/check-host.sh` failed on the new rejection before the fix; it is rerun after the fix along with formatting, note validation, and the quick gate.

## Consequences

Malformed `Connection` values no longer pass the WebSocket upgrade check while valid token lists remain accepted.
