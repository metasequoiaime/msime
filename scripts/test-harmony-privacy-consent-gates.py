#!/usr/bin/env python3
"""鸿蒙的每条联网路径都先问隐私政策的授权。

规则在 `platforms/harmony/entry/src/main/ets/privacy/PrivacyConsentPolicy.ts`，由 `platforms/harmony/tests/privacy-consent.test.ts` 测：哪些桥接请求要联网、什么时候弹窗、版本更新后重新弹窗。把规则接到调用点的是 `.ets` 文件，node 下编不了它们，改回去测试也看不见。这个检查盯住调用点：每条会联网的路径在发出请求之前先经 `HarmonyPrivacyConsent` 放行，设置应用在页面显示时弹窗，两个进程都在动用网络之前读取授权。

文本检查不是编译器：它按方法签名找到方法体，只要求门禁出现在方法体里、并且在第一次联网调用之前。
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ETS = ROOT / "platforms/harmony/entry/src/main/ets"


def method_body(text: str, signature: str) -> str | None:
    """`signature` 之后第一对花括号里的内容。跳过字符串和 `//` 注释里的括号。"""
    start = text.find(signature)
    if start < 0:
        return None
    index = text.find("{", start + len(signature) - 1)
    if index < 0:
        return None
    depth = 0
    position = index
    quote = ""
    while position < len(text):
        character = text[position]
        if quote:
            if character == "\\":
                position += 2
                continue
            if character == quote:
                quote = ""
        elif character in "'\"`":
            quote = character
        elif text.startswith("//", position):
            newline = text.find("\n", position)
            position = len(text) if newline < 0 else newline
            continue
        elif character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0:
                return text[index : position + 1]
        position += 1
    return None


def gated_before(body: str, gate: str, call: str) -> bool:
    gate_at = body.find(gate)
    call_at = body.find(call)
    return gate_at >= 0 and call_at >= 0 and gate_at < call_at


def main() -> int:
    if not ETS.is_dir():
        print("harmony privacy consent gates: no HarmonyOS sources, skipped")
        return 0
    allowed = "HarmonyPrivacyConsent.networkAllowed()"
    wait = "await HarmonyPrivacyConsent.whenNetworkAllowed()"
    # (文件, 方法签名, 门禁, 被它挡住的联网调用)
    gates = [
        ("pages/Settings.ets", "  startRequest(kind: string, id: number, payload: string): string",
         "PrivacyConsentPolicy.refuses(kind, payload, HarmonyPrivacyConsent.networkAllowed())",
         "if (kind === 'account')"),
        ("pages/Settings.ets", "  dictionary(action: string): string",
         "PrivacyConsentPolicy.dictionaryActionNeedsConsent(action)", "KeyboardSession.shared.dictionary("),
        ("pages/Settings.ets", "  private loadNotices(): void", allowed, "client.notices("),
        ("telemetry/Telemetry.ets", "  private static send(request: TelemetryRequest): void", "HarmonyTelemetry.reportingAllowed()",
         "client.telemetryFlush("),
        # 遥测单独使用合并政策与统计开关的共享授权；普通联网功能仍用原政策门禁。
        ("telemetry/Telemetry.ets", "  private static refresh(): void",
         "HarmonyTelemetry.publishConsent()", "client.telemetryBegin("),
        ("telemetry/Telemetry.ets", "  private static publishConsent(): boolean",
         "policy_allowed: HarmonyPrivacyConsent.networkAllowed()", "client.telemetryConsent("),
        ("account/HarmonyAccountIdentity.ets", "  static async ensureAnonymous(directory: string): Promise<void>",
         wait, "client.ensureAnonymousAccountWithAgent("),
        ("keyboard/KeyboardSession.ets",
         "  private async fetchCloud(handle: number, queryDocument: string, epoch: number): Promise<boolean>",
         allowed, "http.createHttp()"),
        ("keyboard/KeyboardSession.ets", "  private async fetchAi(handle: number, queryDocument: string, limit: number,",
         allowed, "http.createHttp()"),
        ("keyboard/KeyboardSession.ets",
         "  private async translationHttp(descriptor: TranslationHttpDescriptor, allowHttp: boolean,",
         allowed, "http.createHttp()"),
        ("keyboard/KeyboardSession.ets", "  async thoughtfulReplies(source: string, style: string,",
         allowed, "http.createHttp()"),
        ("keyboard/clipboard/CloudClipboardClient.ets", "  private async send(action: string): Promise<string>",
         allowed, "client.accountCloudClipboard("),
        # 键盘经自建中继或 WebDAV 网盘的云剪贴板，与账号路径同一道门。
        ("keyboard/clipboard/CloudClipboardClient.ets",
         "  private async sendRelay(action: string, revision: number): Promise<string>",
         allowed, "client.relayCloudClipboard("),
        ("keyboard/input/HarmonyVoicePolisher.ets",
         "  async polish(config: VoiceInputConfiguration, text: string): Promise<string | null>",
         allowed, "http.createHttp()"),
        ("keyboard/input/HarmonyVoiceRecognizer.ets", "  async start(language: string, config: VoiceInputConfiguration,",
         allowed, "await this.doubao.start("),
        ("keyboard/input/HarmonyVoiceRecognizer.ets", "  async start(language: string, config: VoiceInputConfiguration,",
         allowed, "await this.httpAsr.start("),
    ]
    problems: list[str] = []
    for relative, signature, gate, call in gates:
        text = (ETS / relative).read_text(encoding="utf-8")
        body = method_body(text, signature)
        if body is None:
            problems.append(f"{relative}: `{signature.strip()}` not found")
        elif not gated_before(body, gate, call):
            problems.append(f"{relative}: `{call}` is not behind `{gate}`")

    # 两个进程都在动用网络之前读取授权。
    loads = [
        ("entryability/EntryAbility.ets", "  onCreate(want: Want, launch: AbilityConstant.LaunchParam): void",
         "HarmonyPrivacyConsent.load(", "HarmonyTelemetry.applicationStarted("),
        ("inputmethodextability/KeyboardExtensionAbility.ets", "  async onCreate(want: Want): Promise<void>",
         "HarmonyPrivacyConsent.load(state)", "HarmonyAccountIdentity.ensureAnonymous(state)"),
        ("keyboard/KeyboardSession.ets", "    this.syncSharedState();\n    // 隐私政策",
         "HarmonyPrivacyConsent.load(this.stateDirectory)", "this.reloadChangedPreferences()"),
    ]
    for relative, anchor, load, before in loads:
        text = (ETS / relative).read_text(encoding="utf-8")
        start = text.find(anchor)
        scope = text[start:] if start >= 0 else ""
        if start < 0 or not gated_before(scope, load, before):
            problems.append(f"{relative}: `{load}` must run before `{before}`")

    # 设置应用在页面显示时弹窗，并把授权交给键盘和页面。
    settings = (ETS / "pages/Settings.ets").read_text(encoding="utf-8")
    show = method_body(settings, "  onPageShow(): void")
    if show is None or "HarmonyPrivacyConsent.shouldPrompt()" not in show or "this.openPrivacyDialog(true)" not in show:
        problems.append("pages/Settings.ets: onPageShow must open the consent dialog when HarmonyPrivacyConsent.shouldPrompt()")
    decide = method_body(settings, "  decidePrivacy(granted: boolean): void")
    if decide is None or not all(
        part in decide
        for part in ("HarmonyPrivacyConsent.decide(", "this.publishShared()", "msimeHarmonyPrivacyConsentChanged")
    ):
        problems.append("pages/Settings.ets: decidePrivacy must save the decision, publish it to the keyboard and tell the page")
    if "'privacyConsent'" not in settings:
        problems.append("pages/Settings.ets: privacyConsent is not registered on the bridge")

    if problems:
        for problem in problems:
            print(f"harmony privacy consent gate missing: {problem}")
        return 1
    print(f"harmony privacy consent gates: {len(gates)} network paths wait for consent; both processes read it first")
    return 0


if __name__ == "__main__":
    sys.exit(main())
