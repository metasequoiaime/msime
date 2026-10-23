from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess

root = Path.cwd()
folder = root / "target/collocation-tail-evidence"
folder.mkdir(parents=True, exist_ok=True)
allocator = root / "crates/engine/src/ime/personal_rerank.rs"
original = allocator.read_bytes()
needle = b"    #[global_allocator]\n"
assert original.count(needle) == 1
try:
    allocator.write_bytes(original.replace(needle, b"    #[allow(dead_code)]\n"))
    with (folder / "system-build.jsonl").open("w") as output, (folder / "system-build.log").open("w") as errors:
        subprocess.run(
            ["cargo", "test", "--locked", "--release", "-p", "msime-engine", "--lib", "--no-run", "--message-format=json"],
            stdout=output, stderr=errors, check=True,
            env={**os.environ, "CARGO_BUILD_JOBS": "4"},
        )
    records = [json.loads(line) for line in (folder / "system-build.jsonl").read_text().splitlines()]
    binaries = [r["executable"] for r in records if r.get("reason") == "compiler-artifact" and r.get("target", {}).get("name") == "msime_engine" and r.get("executable")]
    assert len(binaries) == 1, binaries
    binary = folder / "paired-system"
    shutil.copy2(binaries[0], binary)
    (folder / "system-binary.json").write_text(json.dumps({
        "allocator": "std::alloc::System",
        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "allocator_source_sha256": hashlib.sha256(original).hexdigest(),
        "decode_sha256": hashlib.sha256((root / "crates/engine/src/lattice/decode.rs").read_bytes()).hexdigest(),
    }, indent=2) + "\n")
finally:
    allocator.write_bytes(original)
assert allocator.read_bytes() == original
