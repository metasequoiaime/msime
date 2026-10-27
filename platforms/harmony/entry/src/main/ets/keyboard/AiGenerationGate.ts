/**
 * AI 生成类功能的总闸，键盘与设置桥一侧。
 *
 * AI 生成类功能指把用户的文字发给大模型、再把生成的内容展示或上屏：高情商回复、AI 润色、AI 联想候选、AI 候选翻译（含 `/fy`）、语音识别后的 AI 润色，以及设置页的 AI 对话、AI 皮肤、AI 测试和模型列表。华为应用市场（中国大陆）要求这类功能先做实名认证、提交《安全评估报告》和生成内容标识材料，资质到位之前鸿蒙版不提供它们，见 `.agents/notes/implemented/feature/2026-10-27-harmony-hide-ai-generation.md`。
 *
 * 开关本身不在这里：它是 Rust 的 `HostCapabilities.ai_generation`（`crates/client-core/src/host_surface.rs`），设置页的 React 一侧读同一个字段。键盘和设置进程各自启动时把 `hostCapabilities` 的回答交给 `adopt`；资质到位后只改 Rust 那一行，两边一起打开。
 *
 * 在 `adopt` 之前、或回答读不懂时按关闭处理：宁可少一个功能，也不在资质没到位时把用户的文字发出去。系统语音识别、云候选和腾讯云、小牛、DeepL 这类机器翻译不是大模型生成，不归这里管。
 */
export class AiGenerationGate {
  /** 设置桥上整个都属于 AI 生成类功能的请求：AI 对话、AI 皮肤、AI 测试和模型列表。 */
  static readonly SETTINGS_REQUESTS: readonly string[] = [
    "chat",
    "ai_skin",
    "ai_test",
    "ai_models",
  ];
  /** 账号通道上的 AI 对话操作（`PAGE_ACCOUNT_OPERATIONS` 里的两项），页面也可能经 `account` 而不是 `chat` 发它们。 */
  static readonly ACCOUNT_OPERATIONS: readonly string[] = ["chat_models", "chat"];
  /** 配置测试里探 AI 服务的几类：它们向大模型接口发请求。 */
  static readonly CREDENTIAL_SERVICES: readonly string[] = [
    "ai.assistant",
    "voice.polish",
    "translation.ai",
  ];
  /** 设置桥拒绝时给页面的错误码。 */
  static readonly REFUSAL: string = "ai_generation_unavailable";

  private static open: boolean = false;

  /** `hostCapabilities` 的回答（`{ok, value}` 信封）里是否打开了 AI 生成类功能。只有明确为 true 才算打开。 */
  static fromCapabilities(document: string): boolean {
    try {
      const reply: Record<string, Object> = JSON.parse(document) as Record<string, Object>;
      if (reply === null || typeof reply !== "object" || reply["ok"] !== true) return false;
      const value: Object | undefined = reply["value"];
      if (value === null || typeof value !== "object") return false;
      return (value as Record<string, Object>)["ai_generation"] === true;
    } catch {
      return false;
    }
  }

  /** 进程启动时读一次宿主能力；返回采用后的结果。 */
  static adopt(document: string): boolean {
    AiGenerationGate.open = AiGenerationGate.fromCapabilities(document);
    return AiGenerationGate.open;
  }

  /** 这个进程现在是否提供 AI 生成类功能。 */
  static enabled(): boolean {
    return AiGenerationGate.open;
  }

  /** 设置桥是否放行这次请求：总闸关着时拒绝 AI 对话、AI 皮肤、AI 测试、模型列表和探 AI 服务的配置测试，其余照常。 */
  static allowsSettingsRequest(kind: string, payload: string): boolean {
    if (AiGenerationGate.open) return true;
    if (AiGenerationGate.SETTINGS_REQUESTS.includes(kind)) return false;
    if (kind === "account") {
      return !AiGenerationGate.ACCOUNT_OPERATIONS.includes(
        AiGenerationGate.field(payload, "operation"),
      );
    }
    if (kind === "api_credential") {
      return !AiGenerationGate.CREDENTIAL_SERVICES.includes(
        AiGenerationGate.field(payload, "service"),
      );
    }
    return true;
  }

  /** 设置桥拒绝时交给页面的回答，与其他请求失败同一个信封。 */
  static refusal(): string {
    return JSON.stringify({ ok: false, error: AiGenerationGate.REFUSAL });
  }

  private static field(payload: string, name: string): string {
    try {
      const document: Record<string, Object> = JSON.parse(payload) as Record<string, Object>;
      if (document === null || typeof document !== "object") return "";
      const value: Object | undefined = document[name];
      return typeof value === "string" ? value : "";
    } catch {
      return "";
    }
  }
}
