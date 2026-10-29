#!/usr/bin/env python3
"""运行 iOS 统计页实际 CSV 导出函数，检查分享快照彼此独立。"""
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "platforms/ios/App/Sources/settings/TypingStatisticsView.swift"


def main():
    swiftc = shutil.which("swiftc")
    if platform.system() != "Darwin" or not swiftc:
        print("skipped: iOS CSV 导出检查需要 macOS 和 swiftc")
        return
    source = SOURCE.read_text()
    start = source.index("  static func writeExport(")
    opening = source.index("{", start)
    depth = 1
    end = opening + 1
    while depth:
        if source[end] == "{":
            depth += 1
        elif source[end] == "}":
            depth -= 1
        end += 1
    # 页面依赖 iOS SwiftUI；只编译原函数，统计格式化用合成字符串替代。改写临时根以免接触宿主文件，写入和读取仍走真实 Foundation。
    function = source[start:end].replace("FileManager.default.temporaryDirectory", "fixtureRoot")
    output = ROOT / "target/ios-statistics-export-tests"
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="case-", dir=output) as folder:
        folder = Path(folder)
        fixture = folder / "main.swift"
        fixture.write_text('''import Foundation
let fixtureRoot = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
struct TypingStatistics {
  let csv: String
  var beforeFormatting: () -> Void = {}
  func dailyCSV() -> String { beforeFormatting(); return csv }
}
struct TypingDailyDetailView {
''' + function + '''
}
func check(_ condition: @autoclosure () -> Bool, _ message: String) {
  guard condition() else { fatalError(message) }
}
let first = TypingStatistics(csv: "\\u{feff}日期,字数\\r\\n2026-10-28,1\\r\\n")
let second = TypingStatistics(csv: "\\u{feff}日期,字数\\r\\n2026-10-29,2\\r\\n")
let firstFile = TypingDailyDetailView.writeExport(first)!
let secondFile = TypingDailyDetailView.writeExport(second)!
check(firstFile != secondFile, "separate exports must have separate URLs")
let attributes = try FileManager.default.attributesOfItem(atPath: firstFile.deletingLastPathComponent().path)
check((attributes[.posixPermissions] as? NSNumber)?.intValue == 0o700, "snapshot directory is private")
let firstBytes = try Data(contentsOf: firstFile)
let secondBytes = try Data(contentsOf: secondFile)
check(firstBytes == Data(first.csv.utf8), "preparing another export must retain the first snapshot")
check(secondBytes == Data(second.csv.utf8), "second export retains its own CSV bytes")
check(firstFile.lastPathComponent == "水杉IME-打字统计.csv", "first export retains the visible filename")
check(secondFile.lastPathComponent == firstFile.lastPathComponent, "random directory must not change the visible filename")
try FileManager.default.removeItem(at: secondFile.deletingLastPathComponent())
check(try! Data(contentsOf: firstFile) == firstBytes, "cleaning one export must not remove another snapshot")
try FileManager.default.removeItem(at: firstFile.deletingLastPathComponent())
// 在目录创建后撤掉写权限，实际文件写入失败时不残留这一笔空目录。
let unwritable = TypingStatistics(csv: first.csv, beforeFormatting: {
  let directories = try! FileManager.default.contentsOfDirectory(at: fixtureRoot, includingPropertiesForKeys: nil)
  check(directories.count == 1, "only the pending failed export has a staging directory")
  try! FileManager.default.setAttributes([.posixPermissions: 0o500], ofItemAtPath: directories[0].path)
})
check(TypingDailyDetailView.writeExport(unwritable) == nil, "CSV write failure must not expose a URL")
check(try! FileManager.default.contentsOfDirectory(at: fixtureRoot, includingPropertiesForKeys: nil).isEmpty,
      "CSV write failure must remove its staging directory")
// 临时根不再是目录时拒绝导出，不删除挡在那里的原文件。
try FileManager.default.removeItem(at: fixtureRoot)
let sentinel = Data("synthetic-existing-file".utf8)
try sentinel.write(to: fixtureRoot)
check(TypingDailyDetailView.writeExport(first) == nil, "unwritable temporary root must fail without exposing a URL")
check(try! Data(contentsOf: fixtureRoot) == sentinel, "failure must preserve the existing file")
print("iOS statistics CSV export: independent snapshots, private permissions, Unicode bytes, filenames and failure preservation passed")
''')
        cache = folder / "cache"
        cache.mkdir()
        executable = folder / "test"
        subprocess.run([swiftc, "-swift-version", "5", str(fixture), "-o", str(executable)],
                       cwd=ROOT, check=True, timeout=60)
        subprocess.run([str(executable), str(cache)], cwd=ROOT, check=True, timeout=15)


if __name__ == "__main__":
    main()
