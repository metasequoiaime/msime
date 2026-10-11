#!/usr/bin/env python3
"""IBus voice workers must pass their cancellation token into provider reads."""

from pathlib import Path

source = Path(__file__).resolve().parents[1] / "platforms/linux/src/core/ClientEngine.cpp"
text = source.read_text()

if "std::function<bool()> cancelled" not in text[text.index("struct VoiceStreamContext"):text.index("// English mode")]:
    raise SystemExit("IBus voice stream context has no cancellation probe")
if "voice_provider_cancelled" not in text:
    raise SystemExit("missing IBus voice cancellation callback")
if "msime_client_voice_provider_stream_feedback_cancelled" not in text:
    raise SystemExit("IBus voice stream does not pass cancellation to the host API")

print("linux IBus voice cancellation: worker token reaches provider stream")
