#!/usr/bin/env python3
"""The Windows installer must say what is missing before it installs.

Neither the WebView2 Runtime nor the Visual C++ redistributable ships in the
package: the first has its own evergreen update channel, the second is a
system-wide shared component. Without either one the input method is broken
*after* a successful install — the Server will not start, or the settings
window opens onto nothing — and the user has no way to know why.

The check lives in Inno Setup's Pascal Script, which nothing on a non-Windows
host can compile. So this pins the parts that are easy to get wrong and that a
later edit could quietly drop, while a Windows run is still what proves it
compiles and behaves.

The call checks read only the prerequisite code - from `function ReadWebView2Version` to the end of `InitializeSetup` - with Pascal comments removed, and each routine by its own body. Searching the whole script let the uninstaller's `RegDeleteKeyIncludingSubkeys(HKLM64, ...)` and a comment that names both views satisfy the registry-view requirement with the real calls deleted.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SETUP = ROOT / "platforms/windows/installer/msime_setup.iss"


def strip_pascal_comments(code: str) -> str:
    """Drop `{ }`, `(* *)` and `//` comments, leaving `'...'` string literals (which may contain `//` in a URL) intact."""
    out: list[str] = []
    index = 0
    while index < len(code):
        char = code[index]
        if char == "'":
            end = code.find("'", index + 1)
            end = len(code) if end < 0 else end + 1
            out.append(code[index:end])
            index = end
        elif char == "{":
            end = code.find("}", index + 1)
            index = len(code) if end < 0 else end + 1
            out.append(" ")
        elif code.startswith("(*", index):
            end = code.find("*)", index + 2)
            index = len(code) if end < 0 else end + 2
            out.append(" ")
        elif code.startswith("//", index):
            end = code.find("\n", index)
            index = len(code) if end < 0 else end
        else:
            out.append(char)
            index += 1
    return "".join(out)


def prerequisite_code(script: str) -> str:
    """From `function ReadWebView2Version` to the `end;` closing `InitializeSetup`, comments removed; empty if either end is missing."""
    start = script.find("function ReadWebView2Version")
    setup = script.find("function InitializeSetup: Boolean;", max(start, 0))
    if start < 0 or setup < 0:
        return ""
    end = re.compile(r"^end;", re.M).search(script, setup)
    if end is None:
        return ""
    return strip_pascal_comments(script[start : end.end()])


def routine(code: str, name: str) -> str:
    """The text of one top-level function in `code`, from its header to its closing `end;`."""
    match = re.search(rf"^function {name}\b.*?^end;", code, re.M | re.S)
    return match.group(0) if match else ""


def main() -> int:
    script = SETUP.read_text(encoding="utf-8")
    failures: list[str] = []

    def require(condition: bool, message: str) -> None:
        if not condition:
            failures.append(message)

    require("function InitializeSetup: Boolean;" in script,
            "no InitializeSetup: nothing runs before the first wizard page")
    code = prerequisite_code(script)
    require(code != "", "no ReadWebView2Version ... InitializeSetup block to check")
    webview = routine(code, "ReadWebView2Version")
    view = routine(code, "VCRuntimeInstalledInView")
    installed = routine(code, "VCRuntimeIsInstalled")
    setup = routine(code, "InitializeSetup")

    # Registry, not the filesystem. Setup.exe is a 32-bit process and Pascal
    # Script's FileExists is redirected to SysWOW64, so probing for
    # vcruntime140.dll reports the x86 runtime and calls an x64-only machine
    # unequipped. Both registry views are named explicitly instead.
    require("Software\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\x64" in script,
            "VC runtime is not read from its x64 registry key")
    require("Software\\Microsoft\\VisualStudio\\14.0\\VC\\Runtimes\\x86" in script,
            "VC runtime is not read from its x86 registry key")
    require("EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" in script,
            "WebView2 is not read from the EdgeUpdate client key")
    require(re.search(r"FileExists\([^)]*vcruntime", script, re.I) is None,
            "VC runtime probed through the filesystem, which WOW64 redirects")
    for root in ("HKLM32", "HKLM64"):
        require(re.search(rf"RegQueryStringValue\(\s*{root}\s*,\s*WebView2ClientKey\s*,\s*'pv'", webview) is not None,
                f"ReadWebView2Version does not read WebView2's pv from the {root} view")
        for key in ("VCRuntimeKey", "VCRuntimeX86Key"):
            require(re.search(rf"VCRuntimeInstalledInView\(\s*{root}\s*,\s*{key}\s*\)", installed) is not None,
                    f"VCRuntimeIsInstalled does not consult {key} in the {root} view")
    require("Result := Result and" in installed,
            "x64 install does not require both x86 and x64 runtimes")
    require("#ifndef Windows7X86" in installed,
            "x86-only Windows 7 install incorrectly requires the x64 runtime")
    require(re.search(r"\bReadWebView2Version\b", setup) is not None,
            "InitializeSetup does not read the WebView2 version")
    require(re.search(r"\bVCRuntimeIsInstalled\b", setup) is not None,
            "InitializeSetup does not check the VC runtime")

    # Installed=1 is true on a machine with only the 2015/2017 redistributable,
    # which lacks vcruntime140_1.dll and still cannot start the Server.
    require(re.search(r"Minor\s*>=\s*20", view) is not None,
            "VC runtime accepted without requiring 14.20 or newer")
    require(re.search(r"RegQueryDWordValue\(\s*RootKey\s*,\s*RuntimeKey\s*,\s*'Installed'", view) is not None,
            "VC runtime Installed value is not read")

    # A silent install is CI or a fleet deployment; a modal dialog there is a
    # hang. Default to continuing, with the missing component in the log.
    require("SuppressibleMsgBox" in setup,
            "prerequisite prompt would block a silent install")
    require(re.search(r"SuppressibleMsgBox\([^;]*IDNO\)", setup, re.S) is not None,
            "silent install does not default to continuing")
    require(setup.count("Log('Prerequisite missing:") == 2,
            "both missing prerequisites must be recorded in the install log")

    # The install is not blocked: a user may be about to fetch the component.
    require("MB_YESNO" in setup, "prerequisite prompt is not a yes/no choice")

    for url in ("https://developer.microsoft.com/zh-cn/microsoft-edge/webview2",
                "https://learn.microsoft.com/zh-cn/cpp/windows/latest-supported-vc-redist",
                "https://aka.ms/vs/17/release/vc_redist.x86.exe",
                "https://aka.ms/vs/17/release/vc_redist.x64.exe"):
        require(url in script, f"missing download page: {url}")
    require(re.search(r"'http://", script) is None,
            "a download page is offered over plain http")

    for failure in failures:
        print(f"FAIL {failure}", file=sys.stderr)
    if failures:
        return 1
    print("installer prerequisites: WebView2 and the VC runtime are checked before install")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
