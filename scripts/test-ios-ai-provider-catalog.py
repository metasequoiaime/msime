#!/usr/bin/env python3
"""用合成静态目录编译真实 iOS AI 枚举，检查接线与平台策略。"""
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    swift = shutil.which("swiftc")
    if platform.system() != "Darwin" or not swift:
        print("skipped: iOS AI 目录接线回归需要 macOS 和 Swift")
        return
    output = ROOT / "target/ios/derived-tests/ai-provider-catalog"
    output.mkdir(parents=True, exist_ok=True)
    source = (ROOT / "platforms/ios/App/Services/CustomService.swift").read_text()
    # 只编译原文件中语音枚举之前的完整声明，不引入语音传输、钥匙串或网络依赖。
    (output / "Presets.swift").write_text(source[:source.index("enum VoiceProviderPreset:")])
    (output / "Catalog.swift").write_text('''
enum AIProviderCatalog {
  struct Provider: Sendable {
    let title: String
    let endpoint: String
    let models: [String]
    let documentation: String
  }
  static func provider(id: String) -> Provider? {
    switch id {
    case "openai": Provider(title: "合成𠮷服务", endpoint: "https://example.invalid/openai",
                            models: ["建议二", "默认𠮷"], documentation: "https://example.invalid/docs")
    case "groq": Provider(title: "合成 Groq", endpoint: "https://example.invalid/groq",
                          models: ["shared-model"], documentation: "https://example.invalid/shared")
    case "custom": Provider(title: "合成自定义", endpoint: "", models: [], documentation: "")
    default: nil
    }
  }
}
''')
    (output / "main.swift").write_text('''
import Foundation
var failures = 0
@MainActor
func expect<T: Equatable>(_ actual: T, _ expected: T, _ message: String) {
  if actual != expected {
    print("FAIL: \\(message)")
    failures += 1
  }
}
expect(AIProviderPreset.openAI.title, "合成𠮷服务", "标题读取共享目录")
expect(AIProviderPreset.openAI.endpoint, "https://example.invalid/openai", "地址读取共享目录")
expect(AIProviderPreset.openAI.models, ["建议二", "默认𠮷"], "模型顺序读取共享目录")
expect(AIProviderPreset.openAI.documentation?.absoluteString, "https://example.invalid/docs", "文档读取共享目录")
expect(AIProviderPreset.anthropic.endpoint, "", "缺失服务不恢复手写地址")
expect(AIProviderPreset.custom.title, "合成自定义", "自定义标题读取共享目录")
expect(AIProviderPreset.custom.models, [], "自定义模型保持空")
expect(AIProviderPreset.custom.documentation?.absoluteString, nil, "自定义文档保持 nil")
expect(AIProviderPreset.groq.title, "合成 Groq", "Groq 标题共享")
expect(AIProviderPreset.groq.endpoint, "https://example.invalid/groq", "Groq 地址共享")
expect(AIProviderPreset.groq.models, ["llama-3.3-70b-versatile", "openai/gpt-oss-120b"], "Groq 模型保留 iOS 策略")
expect(AIProviderPreset.groq.documentation?.absoluteString, "https://console.groq.com/docs/openai", "Groq 文档保留 iOS 策略")
expect(AIProviderPreset.allCases.map(\\.rawValue),
       ["everyAPI", "openAI", "anthropic", "gemini", "deepSeek", "qwen", "kimi", "zhipu", "siliconFlow", "groq", "openRouter", "custom"],
       "枚举顺序、落盘标识和 Ollama 排除策略保持")
if failures != 0 { exit(1) }
print("iOS AI 目录接线与平台策略通过")
''')
    executable = output / "probe"
    subprocess.run([swift, "-swift-version", "6", "-strict-concurrency=complete",
                    str(output / "Presets.swift"), str(output / "Catalog.swift"),
                    str(output / "main.swift"), "-o", str(executable)],
                   cwd=ROOT, check=True, timeout=120)
    subprocess.run([str(executable)], cwd=ROOT, check=True, timeout=30)


if __name__ == "__main__":
    main()
