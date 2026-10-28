#!/usr/bin/env python3
"""编译完整 iOS 采集源码，运行系统栈格式与原 UUID 的真实 Swift 回归。"""
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    if platform.system() != "Darwin" or not shutil.which("swiftc") or not shutil.which("xcrun"):
        print("skipped: iOS MetricKit 栈回归需要 macOS 和 Xcode")
        return
    sdk = subprocess.run(["xcrun", "--sdk", "macosx", "--show-sdk-platform-path"],
                         text=True, capture_output=True, timeout=30)
    if sdk.returncode:
        print("skipped: iOS MetricKit 栈回归需要完整 Xcode 的 macOS 平台")
        return
    sdk_platform = Path(sdk.stdout.strip())
    frameworks = sdk_platform / "Developer/Library/Frameworks"
    private_frameworks = sdk_platform / "Developer/Library/PrivateFrameworks"
    swift_libraries = sdk_platform / "Developer/usr/lib"
    if not (frameworks / "XCTest.framework").is_dir() or not (private_frameworks / "XCTestCore.framework").is_dir():
        print("skipped: iOS MetricKit 栈回归需要 Xcode 的 XCTest.framework")
        return
    output = ROOT / "target/ios/derived-tests"
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="metrickit-frames-", dir=output) as directory:
        directory = Path(directory)
        (directory / "Dependencies.swift").write_text('''
import Foundation
// 只替代容器与偏好桥；解析器和 XCTest 均编译仓库完整源码。
enum InputSchemePreference {
  static let appGroupIdentifier = "app.msime.synthetic.metrickit"
}
enum MetasequoiaInputSessionBridge {
  static let sharedStateDirectory = "/synthetic/preferences"
  static func loadSharedPreferences(stateRoot: URL? = nil) -> [String: Any]? {
    preconditionFailure("栈格式回归不应读取偏好")
  }
  static func updateSharedPreferences(stateRoot: URL? = nil,
      _ mutate: (inout [String: Any]) -> Void) -> Bool {
    preconditionFailure("栈格式回归不应写入偏好")
  }
}
''')
        # 非解析路径全部立即失败，防止测试意外初始化采集、读写用户状态或联网。
        symbols = [
            "msime_client_telemetry_consent", "msime_client_telemetry_begin",
            "msime_client_telemetry_end", "msime_client_telemetry_record_crash",
            "msime_client_telemetry_flush", "msime_client_telemetry_clear",
            "msime_client_notices", "msime_client_notice_dismiss",
        ]
        stubs = "#include <stdlib.h>\n#include <stdint.h>\n"
        stubs += "\n".join(f"char *{name}(const uint8_t *data, unsigned long size) {{ abort(); }}" for name in symbols)
        stubs += "\nvoid msime_client_string_free(char *data) { abort(); }\n"
        stubs += "void msime_ios_crash_record_set(const char *path, const char *header) { abort(); }\n"
        (directory / "Stubs.c").write_text(stubs)
        subprocess.run(["xcrun", "clang", "-c", str(directory / "Stubs.c"),
                        "-o", str(directory / "Stubs.o")], check=True, timeout=60)
        cases = [
            "testCallStackTreeBecomesBinaryAndOffsetLinesCrashedThreadFirst",
            "testCallStackTreeIgnoresNonIntegerOffsets",
            "testMetricKitWrappedTreePreservesOriginalModuleUUIDs",
            "testMetricKitFlatTreePreservesUUIDAndRejectsMalformedIdentity",
        ]
        (directory / "main.swift").write_text("import XCTest\nimport Darwin\n" +
            "let suite = XCTestSuite(name: \"MetricKitFrames\")\n" +
            "\n".join(f'suite.addTest(UsageReportingTests(selector: NSSelectorFromString("{case}")))' for case in cases) +
            f'\nsuite.run()\nguard let result = suite.testRun, result.executionCount == {len(cases)}, '
            'result.hasSucceeded else { exit(1) }\n')
        sources = [
            "platforms/ios/SharedUI/core/UsageReporting.swift",
            "platforms/ios/SharedUI/core/ClientRequestEnvelope.swift",
            "platforms/ios/SharedUI/core/SharedNumber.swift",
            "platforms/ios/SharedUI/core/AppNotices.swift",
            "platforms/ios/tests/core/UsageReportingTests.swift",
        ]
        executable = directory / "metrickit-frames"
        subprocess.run(["xcrun", "swiftc", "-swift-version", "5", "-F", str(frameworks),
                        "-F", str(private_frameworks),
                        "-I", str(swift_libraries), "-L", str(swift_libraries),
                        "-Xlinker", "-rpath", "-Xlinker", str(frameworks),
                        "-Xlinker", "-rpath", "-Xlinker", str(private_frameworks),
                        "-Xlinker", "-rpath", "-Xlinker", str(swift_libraries), *sources,
                        str(directory / "Dependencies.swift"), str(directory / "main.swift"),
                        str(directory / "Stubs.o"), "-o", str(executable)],
                       cwd=ROOT, check=True, timeout=120)
        subprocess.run([str(executable)], cwd=ROOT, check=True, timeout=30)


if __name__ == "__main__":
    main()
