#!/usr/bin/env python3
"""编译真实 iOS 方案与语言分组，验证双拼入口、下载词典能力，以及语言行与选项面板标出的方案一致。"""
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    swiftc = shutil.which("swiftc")
    if platform.system() != "Darwin" or not swiftc:
        print("skipped: iOS 语言入口回归需要 macOS 和 swiftc")
        return
    output = ROOT / "target/ios/derived-tests"
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="input-language-", dir=output) as directory:
        directory = Path(directory)
        (directory / "Dependencies.swift").write_text('''
import Foundation
// 只替代应用容器与外部服务，语言分组、字典文件检查和方案特性编译仓库真实源文件。
struct Bundle {
  static var fixtureRoot = FileManager.default.temporaryDirectory
  static var main: Bundle { Bundle(url: fixtureRoot)! }
  let resourceURL: URL?
  let builtInPlugInsURL: URL?
  init?(url: URL) {
    resourceURL = url.appendingPathComponent("Resources")
    builtInPlugInsURL = url.appendingPathComponent("PlugIns")
  }
}
enum MSIMEAppEdition {
  static let defaultScheme = "quanpin"
  static let inputSchemes: [String]? = nil
  static let appGroupIdentifier = "app.msime.synthetic.input-language"
  static func offers(_ scheme: String) -> Bool { true }
}
enum IOSResourcePacks {
  struct Pack { let id: String; let installed: Bool }
  static var installed = false
  static func bundled(_ id: String) -> Bool { false }
  static func list() throws -> [Pack] { [Pack(id: "language-dictionaries", installed: installed)] }
}
enum MetasequoiaInputSessionBridge {
  static func updateSharedPreferences(stateRoot: URL?, _ mutate: (inout [String: Any]) -> Void) -> Bool { false }
  static func schemeMapping(_ scheme: ChineseInputScheme, enabledSchemes: [ChineseInputScheme]) -> ((inout [String: Any]) -> Void)? { nil }
}
''')
        (directory / "main.swift").write_text('''
import Foundation
let root = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
Bundle.fixtureRoot = root
let resources = root.appendingPathComponent("PlugIns/Synthetic.appex/Resources/EngineResources")
try FileManager.default.createDirectory(at: resources, withIntermediateDirectories: true)
var failures = 0
func expect(_ condition: Bool, _ message: String) {
  if !condition { print("FAIL: \\(message)"); failures += 1 }
}
expect(InputLanguage.cantonese.schemes == [.cantonese, .cantoneseShuangpin], "添加语言的粤拼选项必须提供两个入口")
expect(InputLanguage.of(.cantoneseShuangpin) == .cantonese, "粤语双拼必须归入粤拼语言")
expect(InputLanguage.installedDictionarySchemes.isEmpty, "没有词典或下载包时不能提供词典能力")
IOSResourcePacks.installed = true
expect(InputLanguage.installedDictionarySchemes == [.cantonese, .cantoneseShuangpin, .zhuyin, .stroke], "下载语言词典必须同时解锁粤拼和粤语双拼")
for scheme in [ChineseInputScheme.cantonese, .cantoneseShuangpin] {
  expect(scheme.needsLanguageDictionary && scheme.hasSpellingCaret && !scheme.writesChinese, "两种粤拼保留字典与拼写光标能力")
}
expect(ChineseInputScheme.optInSchemes.contains(.cantoneseShuangpin), "粤语双拼仍由用户选择启用")
// 语言行和它的选项面板代表同一个方案：键盘正用着别的语言时，双拼行显示「小鹤」，面板里也要给小鹤打勾，五笔同理。
let shuangpinSchemes = InputLanguage.shuangpin.schemes
expect(ChineseInputScheme.represented(among: shuangpinSchemes, enabled: [.quanpin, .shuangpin], current: .quanpin) == .shuangpin, "键盘在全拼时，双拼面板仍要标出已启用的小鹤")
expect(ChineseInputScheme.represented(among: shuangpinSchemes, enabled: [.quanpin, .shuangpin, .ziranma], current: .ziranma) == .ziranma, "键盘正用着双拼方案时，标出的就是它")
expect(ChineseInputScheme.represented(among: InputLanguage.wubi.schemes, enabled: [.quanpin, .wubi], current: .quanpin) == .wubi, "键盘在全拼时，五笔面板仍要标出五笔")
expect(ChineseInputScheme.represented(among: shuangpinSchemes, enabled: [.quanpin], current: .quanpin) == nil, "双拼一个方案都没启用时不标任何选项")
if failures != 0 { exit(1) }
print("iOS input language: 11 checks passed")
''')
        executable = directory / "input-language"
        sources = [
            "platforms/ios/SharedUI/preferences/InputSchemePreference.swift",
            "platforms/ios/App/Sources/settings/InputLanguage.swift",
        ]
        subprocess.run([swiftc, "-swift-version", "5", *sources,
                        str(directory / "Dependencies.swift"), str(directory / "main.swift"),
                        "-o", str(executable)], cwd=ROOT, check=True, timeout=60)
        subprocess.run([str(executable), str(directory / "Synthetic.app")], cwd=ROOT,
                       check=True, timeout=15)


if __name__ == "__main__":
    main()
