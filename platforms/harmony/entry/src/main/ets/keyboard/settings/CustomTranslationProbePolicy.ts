import { utf8Length } from "../Utf8";

export interface CustomTranslationProbeConfig {
  enabled: boolean;
  endpoint: string;
  api_key: string;
}

/** 交给 NAPI `customTranslationHttpRequest` 的请求，键盘翻译候选词时发的是同一种。 */
export interface CustomTranslationProbeRequest {
  config: CustomTranslationProbeConfig;
  text: string;
  source_language: string;
  target_language: string;
}

/** 共享描述器：URL、请求头和正文都由 Rust 的 `custom_translation_request` 构造。 */
export interface CustomTranslationProbeDescriptor {
  url: string;
  headers: Record<string, string>;
  body: Object;
}

/**
 * 鸿蒙设置页「测试自定义翻译配置」里不碰网络和 NAPI 的部分，从 Settings.ets 拆出来，放进 node 单测（platforms/harmony/tests/custom-translation-probe.test.ts）。
 *
 * 请求本身由共享描述器构造，这里只决定交给它什么、怎么认出 DeepL，以及失败时说什么。DeepL 的失败提示与 crates/client-core/src/credential/translation.rs 的 `deepl_failure_message` 逐字相同（Tauri 设置页显示的是那一份）；鸿蒙设置页拿不到那个函数，scripts/test-harmony-custom-translation-probe.py 逐字核对两边。
 */
export class CustomTranslationProbePolicy {
  /** 共享构造给 DeepL 官方 API 写的鉴权头前缀；DeepLX 格式写的是 `Bearer`。 */
  static readonly DEEPL_AUTHORIZATION_PREFIX: string = "DeepL-Auth-Key ";
  /** 回复正文的上限，超过就不交给共享解析。 */
  static readonly MAX_RESPONSE_BYTES: number = 256 * 1024;
  static readonly SUCCESS: string = "连接成功，翻译服务配置有效。";
  static readonly GENERIC_FAILURE: string = "测试失败：翻译服务未返回有效译文。";

  /** 与 Rust 设置页连接测试相同的探测：中文「测试」译成英文。地址和密钥由调用方校验并去掉首尾空白后传入。 */
  static request(endpoint: string, apiKey: string): CustomTranslationProbeRequest {
    return {
      config: { enabled: true, endpoint: endpoint, api_key: apiKey },
      text: "测试",
      source_language: "zh",
      target_language: "en",
    };
  }

  /** 按共享描述器的鉴权头认出 DeepL，不在这里再写一份主机名判断。 */
  static isDeepL(descriptor: CustomTranslationProbeDescriptor): boolean {
    const authorization: string | undefined = descriptor.headers["Authorization"];
    return authorization !== undefined
      && authorization.startsWith(CustomTranslationProbePolicy.DEEPL_AUTHORIZATION_PREFIX);
  }

  /** 实际发出的请求头：描述器的全部请求头，另加 `Accept`。返回新对象，不改描述器。 */
  static headers(descriptor: CustomTranslationProbeDescriptor): Record<string, string> {
    const headers: Record<string, string> = {};
    for (const name of Object.keys(descriptor.headers)) {
      headers[name] = descriptor.headers[name];
    }
    headers["Accept"] = "application/json";
    return headers;
  }

  /** 状态码是 2xx、正文是字符串且不超过上限时，才交给共享解析。 */
  static acceptsResponse(status: number, result: Object | string | undefined): boolean {
    return status >= 200 && status < 300 && typeof result === "string"
      && utf8Length(result) <= CustomTranslationProbePolicy.MAX_RESPONSE_BYTES;
  }

  /** 没拿到译文时的提示。DeepL 按状态码区分常见错误，只用状态码，不转述服务端正文；其他服务一律是笼统的提示。 */
  static failureMessage(deepl: boolean, status: number): string {
    if (!deepl) return CustomTranslationProbePolicy.GENERIC_FAILURE;
    if (status === 403) return "测试失败：DeepL 拒绝了这个 API Key（HTTP 403），请检查密钥是否正确、是否已停用。";
    if (status === 456) return "测试失败：DeepL API 的字符额度已用完（HTTP 456）。";
    if (status === 429) return "测试失败：DeepL 请求过于频繁（HTTP 429），请稍后再试。";
    return "测试失败：DeepL 未返回有效译文，请检查 API Key 和网络。";
  }
}
