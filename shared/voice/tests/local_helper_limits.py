"""Input-size limits for the standalone local voice helper."""

import os
import struct
import subprocess
import sys
import tempfile
from pathlib import Path


helper = sys.argv[1]
max_wav_bytes = 44 + 16000 * 60 * 2

with tempfile.TemporaryDirectory() as directory:
    wav = Path(directory) / "oversized.wav"
    data_size = max_wav_bytes - 44
    with wav.open("wb") as output:
        output.write(b"RIFF")
        output.write(struct.pack("<I", 36 + data_size))
        output.write(b"WAVEfmt ")
        output.write(struct.pack("<IHHIIHH", 16, 1, 1, 16000, 32000, 2, 16))
        output.write(b"data")
        output.write(struct.pack("<I", data_size))
        output.truncate(max_wav_bytes + 1)

    result = subprocess.run(
        [helper, "--model", str(Path(directory) / "model"), "--wav", str(wav)],
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode != 0
    assert "too large" in result.stderr.lower(), result.stderr

    if os.name != "nt":
        outside = Path(directory) / "outside.wav"
        outside.write_bytes(
            b"RIFF"
            + struct.pack("<I", 36)
            + b"WAVEfmt "
            + struct.pack("<IHHIIHH", 16, 1, 1, 16000, 32000, 2, 16)
            + b"data"
            + struct.pack("<I", 0)
        )
        linked = Path(directory) / "linked.wav"
        linked.symlink_to(outside)
        result = subprocess.run(
            [helper, "--model", str(Path(directory) / "model"), "--wav", str(linked)],
            capture_output=True,
            text=True,
            check=False,
            timeout=3,
        )
        assert result.returncode != 0
        assert "cannot open" in result.stderr.lower(), result.stderr

        fifo = Path(directory) / "input.fifo"
        os.mkfifo(fifo)
        result = subprocess.run(
            [helper, "--model", str(Path(directory) / "model"), "--wav", str(fifo)],
            capture_output=True,
            text=True,
            check=False,
            timeout=3,
        )
        assert result.returncode != 0
        assert "cannot open" in result.stderr.lower(), result.stderr
