#!/usr/bin/env python3
"""Harmony audio players must bind validated pack paths to the descriptor they use."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MUSIC = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/MusicPlayer.ets"
SOUND = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/KeySoundPlayer.ets"


def main() -> int:
    music = MUSIC.read_text(encoding="utf-8")
    sound = SOUND.read_text(encoding="utf-8")
    music_required = {
        "music path helper checks the directory entry": "private static openVerified(path: string): fs.File" in music
        and "const pathStat: fs.Stat = fs.lstatSync(path);" in music,
        "music path helper binds the opened descriptor": "const opened: fs.Stat = fs.statSync(file.fd);" in music
        and "opened.ino !== pathStat.ino" in music,
        "metadata reads use the verified descriptor": "file = MusicPlayer.openVerified(path);" in music,
        "playback reads use the verified descriptor": "MusicPlayer.openVerified(pack.tracks[index])" in music,
    }
    sound_required = {
        "rendered sound path checks the directory entry": "private static openVerified(path: string): fs.File" in sound
        and "const pathStat: fs.Stat = fs.lstatSync(path);" in sound,
        "rendered sound path binds the opened descriptor": "const opened: fs.Stat = fs.statSync(file.fd);" in sound
        and "opened.ino !== pathStat.ino" in sound,
        "sound pool loads the verified descriptor": "const file: fs.File = KeySoundPlayer.openVerified(path);" in sound,
    }
    missing = [name for name, present in {**music_required, **sound_required}.items() if not present]
    if missing:
        print("harmony audio file binding: missing " + ", ".join(missing))
        return 1
    print("harmony audio players bind pack paths before opening them")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
