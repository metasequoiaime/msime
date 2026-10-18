#!/usr/bin/env python3
"""编译真实模型管理器，以条件同步验证取消、离页和删除后的状态。"""
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    swiftc = shutil.which("swiftc")
    if platform.system() != "Darwin" or not swiftc:
        print("skipped: iOS 模型取消回归需要 macOS 和 swiftc")
        return
    output = ROOT / "target/ios-local-model-cancel"
    output.mkdir(parents=True, exist_ok=True)
    source = (ROOT / "platforms/ios/App/Sources/settings/LocalSpeechModelsView.swift").read_text()
    # 只移除 SwiftUI 视图和导入，管理器逐字编译，避免维护一份测试专用状态机。
    manager, separator, _ = source.partition("\nstruct LocalSpeechModelsSection: View {")
    assert separator, "模型管理器与视图的源码边界已改变"
    manager = manager.replace("import SwiftUI\n", "import Combine\nimport Foundation\n", 1)
    generated = output / "LocalSpeechModelManager.swift"
    generated.write_text(manager)
    sources = [str(generated),
               "platforms/ios/App/Services/LocalSpeechModelDownload.swift",
               "platforms/ios/App/Sources/app/ViewRequestGeneration.swift",
               "platforms/ios/tests/local-models/LocalModelCancellationTests.swift"]
    binary = output / "tests"
    subprocess.run([swiftc, "-swift-version", "5", "-parse-as-library", *sources,
                    "-o", str(binary)], cwd=ROOT, check=True, timeout=60)
    for operation in ["download", "import"]:
        for mode in ["early", "registration", "leave", "blocked-cancel", "progress-blocked"]:
            subprocess.run([str(binary), operation, mode], cwd=ROOT, check=True, timeout=15)
    for mode in ["switch-other", "select-removed", "selected", "unselected", "failure"]:
        subprocess.run([str(binary), "remove", mode], cwd=ROOT, check=True, timeout=15)


if __name__ == "__main__":
    main()
