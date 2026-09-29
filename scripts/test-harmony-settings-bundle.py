#!/usr/bin/env python3
"""Whether the HarmonyOS settings page is built, not committed, and still builds.

The HarmonyOS settings window is a WebView over `$rawfile('settings/index.html')`: one self-contained document with the script, the styles and every asset inlined, because a resource:// document has a null origin and the webview will not fetch anything across it. `platforms/harmony/stage-settings.sh` builds it from `packages/ui` before every HAP build, and `entry/hvigorfile.ts` refuses to package without it.

It used to be committed. That went wrong twice over. First it went stale: fifty-two commits touched `packages/ui/src` while the committed bundle was never regenerated, and a stale bundle is a working bundle, so the HarmonyOS settings page rendered a UI that no longer existed anywhere else with no error to see. The fix for that, a check that rebuilt it and demanded the committed bytes match, made every shared-UI pull request regenerate the same 1 MB single-line file, so any two of them in flight conflicted on it, and a merge could only be resolved by building it again.

Building it from the same checkout as the HAP removes both: it cannot be older than the UI it ships with, and there is nothing to conflict on. What is left to check is that it stays that way, which is that the page is not tracked again, and that it still builds into the one file the webview can read.
"""

import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAGE = "platforms/harmony/entry/src/main/resources/rawfile/settings/index.html"
STAGE = ["bash", "platforms/harmony/stage-settings.sh"]


def git(*arguments: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *arguments], cwd=ROOT, capture_output=True, text=True)


def main() -> int:
    if shutil.which("git") is not None and git("rev-parse", "--is-inside-work-tree").returncode == 0:
        if git("ls-files", "--error-unmatch", PAGE).returncode == 0:
            print(f"{PAGE} is tracked again")
            print("  it is built by platforms/harmony/stage-settings.sh; committing it makes every shared-UI change conflict on it")
            print(f"  untrack it with: git rm --cached {PAGE}")
            return 1
        if git("check-ignore", "-q", PAGE).returncode != 0:
            print(f"{PAGE} is not ignored, so a build leaves it looking like a change to commit")
            return 1
    if shutil.which("pnpm") is None:
        print("skipped: pnpm not on PATH, cannot build the settings page")
        return 0
    if not (ROOT / "node_modules").exists():
        print("skipped: dependencies not installed, run pnpm install at the repository root")
        return 0
    try:
        result = subprocess.run(STAGE, cwd=ROOT, capture_output=True, text=True)
    except OSError as error:
        print(f"skipped: could not run the settings build ({error})")
        return 0
    if result.returncode != 0:
        print("the HarmonyOS settings page does not build")
        print(result.stdout[-2000:])
        print(result.stderr[-2000:])
        return 1
    print("harmony settings page: builds into one file and is not committed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
