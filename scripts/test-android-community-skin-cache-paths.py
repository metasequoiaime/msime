#!/usr/bin/env python3
from pathlib import Path


SOURCE = Path(__file__).parents[1] / "platforms/android/java/app/msime/android/community/CommunitySkinCache.java"
text = SOURCE.read_text()

write_start = text.index("public static void write(")
read_start = text.index("public static List<Entry> read(")
write_body = text[write_start:read_start]
read_body = text[read_start:]
assert "SafePaths.ensureDirectory(preferencesDirectory);" in write_body, (
    "community skin cache writes must validate the preferences directory"
)
assert "SafePaths.rejectSymlinkComponents(preferencesDirectory);" in read_body, (
    "community skin cache reads must validate the preferences directory"
)
print("Android community skin cache paths reject symlinked directories")
