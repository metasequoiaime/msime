import runpy
import subprocess
import tempfile
from pathlib import Path

namespace = runpy.run_path("platforms/linux/tests/core/ibus_launcher_supervisor.py")["main"].__globals__
original_path = namespace["LAUNCHER"]
source = original_path.read_text()
upgrade = 'if [ "$status" -eq "$upgraded_status" ]; then\n'
pause = '  if [ -n "$pause" ]; then kill -TERM "$pause" 2>/dev/null || :; fi\n'
variants = (
    ("wrong-backoff", "min_delay=2\n", "min_delay=3\n"),
    ("upgrade-waits", upgrade, upgrade + "    sleep 2\n"),
    ("pause-not-stopped", pause, ""),
)
scratch_root = Path("target/page-compaction-evidence")
scratch_root.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(dir=scratch_root) as scratch:
    for name, needle, replacement in variants:
        assert source.count(needle) == 1, name
        variant = Path(scratch) / name
        variant.write_text(source.replace(needle, replacement, 1))
        namespace["LAUNCHER"] = variant
        try:
            namespace["main"]()
        except (AssertionError, subprocess.TimeoutExpired) as error:
            print(f"{name}: detected {type(error).__name__}", flush=True)
        else:
            raise AssertionError(f"{name}: mutation was not detected")
namespace["LAUNCHER"] = original_path
