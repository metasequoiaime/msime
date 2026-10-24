from pathlib import Path
import subprocess

root = Path.cwd()
source = Path(__file__).resolve().parent
probe = root / 'target/key-probe'
probe.mkdir(parents=True, exist_ok=True)
for original, name in [('compact-Cargo.toml', 'Cargo.toml'), ('compact-Cargo.lock', 'Cargo.lock')]:
    (probe / name).write_bytes((source / original).read_bytes())
(probe / 'bench.rs').write_text((source / 'compact-bench.rs').read_text().replace('__CACHE__', str(root / 'crates/engine/src/cache.rs')))
subprocess.run(['cargo', 'build', '--release', '--locked', '--manifest-path', str(probe / 'Cargo.toml'), '--target-dir', str(root / 'target')], check=True)
folder = root / 'target/next-hotspot-evidence/compact'
folder.mkdir(parents=True, exist_ok=True)
with (folder / 'bench.jsonl').open('w') as out, (folder / 'bench-run.log').open('w') as err:
    subprocess.run([str(root / 'target/release/key-probe')], stdout=out, stderr=err, check=True)
