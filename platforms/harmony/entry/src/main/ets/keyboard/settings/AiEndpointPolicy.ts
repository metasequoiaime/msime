/**
 * AI 辅助接口地址的传输策略：https 不限主机；http 只能指向本机或局域网，公网主机必须走 https，API Token 不能明文经过互联网。规则只在 Rust（crates/client-providers/src/ai/endpoint.rs），宿主经 NAPI 的 `aiEndpointCheck`（`msime_client_ai_endpoint_check`）询问，见 `AiEndpoint.ets`；这里只读 Rust 的回答，不依赖 NAPI，node 测试也能跑。
 */

/** `allowed`、`invalid` 或 `cleartext_public`，与共用用例 shared/contracts/ai-endpoint/cases.json 的 `result` 相同。 */
export interface AiEndpointCheck {
  result: string;
  /** 保存 Token 用的来源键 `scheme://host:port`；地址不合规时为 null。 */
  origin: string | null;
}

interface AiEndpointAnswer {
  origin?: string | null;
  problem?: string | null;
}

interface AiEndpointReply {
  ok?: boolean;
  value?: AiEndpointAnswer | null;
}

const INVALID: AiEndpointCheck = { result: "invalid", origin: null };

export class AiEndpointPolicy {
  /** 公网 http 地址被拒绝时给用户看的说明。 */
  static readonly CLEARTEXT_HINT: string =
    "http 只能用于本机或局域网地址（如 192.168.x.x、localhost、*.local），公网服务请用 https，以免 API Token 明文经过互联网";

  /** 读 `aiEndpointCheck` 的回答。C 接口拒绝请求或回答读不懂时按不合规处理，不发请求。 */
  static parse(reply: string): AiEndpointCheck {
    let parsed: AiEndpointReply;
    try {
      parsed = JSON.parse(reply) as AiEndpointReply;
    } catch (_) {
      return INVALID;
    }
    if (parsed === null || typeof parsed !== "object" || parsed.ok !== true) return INVALID;
    const value: AiEndpointAnswer | null | undefined = parsed.value;
    if (value === null || value === undefined || typeof value !== "object") return INVALID;
    if (value.problem === null) {
      return typeof value.origin === "string"
        ? { result: "allowed", origin: value.origin }
        : INVALID;
    }
    return value.problem === "cleartext_public"
      ? { result: "cleartext_public", origin: null }
      : INVALID;
  }
}
