"""Keep the macOS local voice helper's line parser bounded before JSON decoding."""

from pathlib import Path


source = Path("platforms/macos/src/voice/LocalVoiceRequest.mm").read_text()
handler = source.split("output.fileHandleForReading.readabilityHandler = ^", 1)[1].split(
    "task.terminationHandler", 1
)[0]

parse = handler.index("NSJSONSerialization JSONObjectWithData:")
guard = handler.index("if (droppingOversizedLine || length > MaximumHelperLine)")
assert guard < parse, "oversized helper lines must be discarded before JSON parsing"
assert "__block BOOL droppingOversizedLine = NO;" in source
assert "droppingOversizedLine = YES;" in handler
