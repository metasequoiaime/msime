#!/usr/bin/env python3
"""Collect the license texts of third-party code built into the Linux and Windows releases.

The shared host library is a Rust cdylib and the desktop settings binary is a Rust binary, so both statically link their crate graphs; the desktop binary also embeds a web frontend bundled from npm packages. MIT, Apache-2.0 and the other licenses in those graphs require their notices to travel with binary copies, and no distribution copyright file covers them because nothing is linked dynamically. package-container.sh runs this script and hands the output to CMake as MSIME_RUST_NOTICES and MSIME_FRONTEND_NOTICES; the Windows release workflow runs it on the Windows runner, so Cargo resolves the Windows graph, and hands the output to Collect-Notices.ps1 as supplemental notices.

  collect-notices.py cargo OUTPUT PACKAGE[:FEATURES] ...   crates reachable through normal, non-proc-macro edges from each workspace package, as Cargo resolves them for the host target
  collect-notices.py npm OUTPUT PACKAGE_DIR                npm packages reachable through dependencies/optionalDependencies from PACKAGE_DIR, resolved through node_modules the way Node resolves them

Workspace members under the project's own GPL-3.0-only are not listed; the members with a license of their own (msime-engine and msime-path-trust under MPL-2.0, msime-script-sdk under MIT) are collected like any other crate, so their LICENSE travels with the binary. Every license, notice or copyright file shipped in a collected package is copied verbatim; packages that ship none are listed with their declared license expression so the gap is visible instead of silently dropped. Only standard-library modules are used and nothing is fetched: the inputs are the already downloaded crates and the installed node_modules.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

# The workspace license; members that declare another one ship their own LICENSE.
PROJECT_LICENSE = "GPL-3.0-only"
LICENSE_FILE = re.compile(r"^(licen[cs]e|copying|copyright|notice|unlicense)([-._].*)?$", re.IGNORECASE)


def license_files(directory: Path) -> list[Path]:
    return sorted(p for p in directory.iterdir() if p.is_file() and LICENSE_FILE.match(p.name))


def write_notices(output: Path, title: str, entries: list[tuple[str, str, str, list[Path]]]) -> None:
    lines = [title, "=" * len(title), "", f"{len(entries)} packages. Each section is the package's own license, notice and copyright files, copied verbatim."]
    missing = [f"* {name} {version} ({expression})" for name, version, expression, files in entries if not files]
    if missing:
        lines += ["", "These packages ship no license file; their declared license expression applies:", *missing]
    for name, version, _, files in entries:
        for path in files:
            lines += ["", "-" * 78, f"{name} {version} — {path.name}", "-" * 78, ""]
            lines.append(path.read_text(encoding="utf-8", errors="replace").rstrip())
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def collect_cargo(output: Path, roots: list[str]) -> None:
    metadata = json.loads(subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        check=True, capture_output=True, text=True).stdout)
    members = set(metadata["workspace_members"])
    packages = {(p["name"], p["version"]): p for p in metadata["packages"]
                if p["id"] not in members or p.get("license") != PROJECT_LICENSE}
    reached: set[tuple[str, str]] = set()
    for root in roots:
        package, _, features = root.partition(":")
        command = ["cargo", "tree", "--locked", "-p", package, "-e", "normal,no-proc-macro",
                   "--prefix", "none", "--format", "{p}"]
        if features:
            command += ["--features", features]
        tree = subprocess.run(command, check=True, capture_output=True, text=True).stdout
        for line in tree.splitlines():
            fields = line.split()
            if len(fields) >= 2 and fields[1].startswith("v") and (fields[0], fields[1][1:]) in packages:
                reached.add((fields[0], fields[1][1:]))
    entries = []
    for key in sorted(reached):
        package = packages[key]
        directory = Path(package["manifest_path"]).parent
        files = license_files(directory)
        if package.get("license_file"):
            declared = (directory / package["license_file"]).resolve()
            if declared.is_file() and declared not in {path.resolve() for path in files}:
                files.append(declared)
        entries.append((key[0], key[1], package.get("license") or "no license declared", files))
    write_notices(output, "Rust crates statically linked into the MSIME host library and binaries", entries)


def resolve_node_package(name: str, start: Path) -> Path | None:
    for directory in [start, *start.parents]:
        candidate = directory / "node_modules" / name
        if (candidate / "package.json").is_file():
            return candidate.resolve()
    return None


def collect_npm(output: Path, root: Path) -> None:
    root = root.resolve()
    seen: set[Path] = set()
    entries = []
    pending = [root]
    while pending:
        directory = pending.pop()
        if directory in seen:
            continue
        seen.add(directory)
        manifest = json.loads((directory / "package.json").read_text(encoding="utf-8"))
        # Workspace packages resolve outside node_modules; they are the project's own code.
        if "node_modules" in directory.parts:
            license_field = manifest.get("license") or manifest.get("licenses") or "no license declared"
            if not isinstance(license_field, str):
                license_field = json.dumps(license_field, ensure_ascii=False)
            entries.append((manifest["name"], manifest.get("version", ""), license_field, license_files(directory)))
        dependencies = {**manifest.get("dependencies", {}), **manifest.get("optionalDependencies", {})}
        for name in dependencies:
            resolved = resolve_node_package(name, directory)
            if resolved is None:
                if name in manifest.get("optionalDependencies", {}):
                    continue
                sys.exit(f"{manifest['name']} depends on {name}, which is not installed; run pnpm install --frozen-lockfile first")
            pending.append(resolved)
    entries.sort(key=lambda entry: (entry[0], entry[1]))
    write_notices(output, "npm packages bundled into the MSIME desktop settings frontend", entries)


def main() -> None:
    if len(sys.argv) >= 4 and sys.argv[1] == "cargo":
        collect_cargo(Path(sys.argv[2]), sys.argv[3:])
    elif len(sys.argv) == 4 and sys.argv[1] == "npm":
        collect_npm(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
