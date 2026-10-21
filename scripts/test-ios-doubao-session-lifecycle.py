#!/usr/bin/env python3
"""在 macOS Foundation 上执行完整豆包传输测试，不构建 iOS 宿主或启动模拟器。"""
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    swift = shutil.which("swift")
    if platform.system() != "Darwin" or not swift:
        print("skipped: iOS 豆包 session 回归需要 macOS 和 Swift")
        return
    output = ROOT / "target/ios/derived-tests/doubao-transport"
    sources = output / "Sources"
    tests = output / "Tests"
    sources.mkdir(parents=True, exist_ok=True)
    tests.mkdir(parents=True, exist_ok=True)
    # 每次重建测试输入，删除上次运行留下的源码，避免删掉的测试继续被执行。
    for directory in (sources, tests):
        for previous in directory.glob("*.swift"):
            previous.unlink()
    for name in ("DoubaoVoiceCoordinator.swift", "DoubaoWebSocketTransport.swift"):
        shutil.copyfile(ROOT / "platforms/ios/App/Services" / name, sources / name)
    for source in (ROOT / "platforms/ios/TransportTests").glob("*.swift"):
        shutil.copyfile(source, tests / source.name)
    (output / "Package.swift").write_text('''// swift-tools-version: 5.9
import PackageDescription
let package = Package(
  name: "MSIMEDoubaoTransport",
  platforms: [.macOS(.v12)],
  targets: [
    .target(name: "MSIMEDoubaoTransport", path: "Sources"),
    .testTarget(name: "MSIMEDoubaoTransportTests", dependencies: ["MSIMEDoubaoTransport"], path: "Tests")
  ]
)
''')
    subprocess.run([swift, "test", "--package-path", str(output), "--jobs", "2"],
                   cwd=ROOT, check=True, timeout=120)


if __name__ == "__main__":
    main()
