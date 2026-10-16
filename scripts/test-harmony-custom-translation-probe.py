#!/usr/bin/env python3
"""鸿蒙设置页「测试自定义翻译配置」与共享层保持一致的守卫。

连接测试里不碰网络和 NAPI 的部分在 `CustomTranslationProbePolicy.ts`，由 platforms/harmony/tests/custom-translation-probe.test.ts 在 node 下逐条跑；那组测试只在鸿蒙 CI 作业里运行。这里不需要任何工具链，每次推送都按文本核对三件事：DeepL 失败提示逐字等于 crates/client-providers/src/credential/translation.rs 的 `deepl_failure_message`（Tauri 设置页显示的就是那一份），两边探测的是同一个请求，以及 Settings.ets 的连接测试仍然走共享描述器、共享解析和这个策略模块，而不是自己拼请求或另写一份文案。
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUST_PROBE = ROOT / "crates/client-providers/src/credential/translation.rs"
RUST_BUILDER = ROOT / "crates/client-core/src/translation.rs"
SETTINGS = ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets"
POLICY = ROOT / "platforms/harmony/entry/src/main/ets/keyboard/settings/CustomTranslationProbePolicy.ts"
NAPI_TYPES = ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts"


def block(text: str, start: str, end: str) -> str:
    """从 `start` 所在处截到其后第一个 `end`；找不到返回空串，由调用方报错。"""
    begin = text.find(start)
    if begin < 0:
        return ""
    finish = text.find(end, begin + len(start))
    return text[begin:] if finish < 0 else text[begin:finish]


def rust_messages(source: str) -> dict[str, str]:
    body = block(source, "fn deepl_failure_message(", "\n}\n")
    messages = {code: text for code, text in re.findall(r'Some\((\d{3})\) => \{?\s*"([^"]+)"', body)}
    fallback = re.search(r'_ => "([^"]+)"', body)
    if fallback:
        messages["other"] = fallback.group(1)
    return messages


def harmony_messages(source: str) -> dict[str, str]:
    body = block(source, "static failureMessage(deepl: boolean, status: number): string {", "\n  }\n")
    messages = {code: text for code, text in re.findall(r'if \(status === (\d{3})\)\s+return "([^"]+)";', body)}
    returns = re.findall(r'^    return "([^"]+)";$', body, re.MULTILINE)
    if returns:
        messages["other"] = returns[-1]
    return messages


def main() -> int:
    missing = [path for path in (RUST_PROBE, RUST_BUILDER, SETTINGS, POLICY, NAPI_TYPES) if not path.is_file()]
    if missing:
        print("harmony custom translation probe: skipped, missing " + ", ".join(str(path.relative_to(ROOT)) for path in missing))
        return 0
    rust_probe = RUST_PROBE.read_text(encoding="utf-8")
    rust_builder = RUST_BUILDER.read_text(encoding="utf-8")
    settings = SETTINGS.read_text(encoding="utf-8")
    policy = POLICY.read_text(encoding="utf-8")
    napi_types = NAPI_TYPES.read_text(encoding="utf-8")
    problems: list[str] = []

    rust = rust_messages(rust_probe)
    harmony = harmony_messages(policy)
    if set(rust) != {"403", "456", "429", "other"}:
        problems.append(f"could not read the four DeepL messages from {RUST_PROBE.relative_to(ROOT)}: {sorted(rust)}")
    elif rust != harmony:
        for key in sorted(set(rust) | set(harmony)):
            if rust.get(key) != harmony.get(key):
                problems.append(f"DeepL {key} message differs: Rust {rust.get(key)!r}, Harmony {harmony.get(key)!r}")

    # 鸿蒙靠描述器的鉴权头认出 DeepL，这个前缀必须就是共享构造写进去的那个。
    if 'format!("DeepL-Auth-Key {api_key}")' not in rust_builder:
        problems.append("the shared builder no longer writes `DeepL-Auth-Key <key>`; update the Harmony DeepL detection with it")
    if 'DEEPL_AUTHORIZATION_PREFIX: string = "DeepL-Auth-Key ";' not in policy:
        problems.append("CustomTranslationProbePolicy no longer recognises DeepL by the `DeepL-Auth-Key ` prefix the shared builder writes")

    # 两边探测同一个请求：中文「测试」译成英文。
    if 'translation::custom_translation_request(endpoint, key, "测试", "zh", "en")' not in rust_probe:
        problems.append("the Rust probe no longer translates 测试 from zh to en; update CustomTranslationProbePolicy.request with it")
    request = block(policy, "static request(endpoint: string, apiKey: string)", "\n  }\n")
    if not all(part in request for part in ('text: "测试"', 'source_language: "zh"', 'target_language: "en"')):
        problems.append("CustomTranslationProbePolicy.request no longer probes 测试 from zh to en like the Rust probe")

    probe = block(settings, "private async translationCredential(config: ApiCredentialConfig)", "\n  private ")
    required = {
        "takes the probe request from the policy": "CustomTranslationProbePolicy.request(endpoint, apiKey)" in probe,
        "builds the request with the shared descriptor": "client.customTranslationHttpRequest(JSON.stringify(probe))" in probe,
        "rejects a missing descriptor": "descriptor === null" in probe,
        "posts to the descriptor URL": "request.request(descriptor.url," in probe,
        "sends the descriptor body": "extraData: JSON.stringify(descriptor.body)" in probe,
        "keeps the descriptor headers and adds Accept": "CustomTranslationProbePolicy.headers(descriptor)" in probe
        and "header: headers," in probe,
        "detects DeepL by the shared Authorization header": "CustomTranslationProbePolicy.isDeepL(descriptor)" in probe,
        "checks the reply before parsing": "CustomTranslationProbePolicy.acceptsResponse(response.responseCode, response.result)" in probe,
        "parses with the shared parser": "client.parseCustomTranslationResponse(response.result as string)" in probe,
        "treats ok:false as a failure": "parsed.ok === true && typeof parsed.value === 'string'" in probe,
        "reports failures by status through the policy": probe.count("CustomTranslationProbePolicy.failureMessage(deepl, response.responseCode)") == 2,
        "does not build its own Bearer header": "Bearer" not in probe,
        "keeps no copy of the DeepL messages": "DeepL API 的字符额度" not in settings and "DeepL 拒绝了" not in settings,
        "NAPI declares the descriptor": "customTranslationHttpRequest" in napi_types,
        "NAPI declares the parser": "parseCustomTranslationResponse" in napi_types,
    }
    problems.extend(f"translationCredential: {name}" for name, present in required.items() if not present)

    if problems:
        for problem in problems:
            print(f"harmony custom translation probe: {problem}")
        return 1
    print("harmony custom translation probe: shared descriptor, shared parser, the probe request and the DeepL messages match the Rust probe")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
