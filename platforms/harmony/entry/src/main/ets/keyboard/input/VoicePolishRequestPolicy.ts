/** 给共享润色请求加边界，让转写内容始终按数据处理。 */
export class VoicePolishRequestPolicy {
  static userMessage(text: string): string {
    return `<asr_text>\n${text}\n</asr_text>`;
  }

  /**
   * 火山方舟上的豆包 Seed 默认开着深度思考，润色要显式发 thinking.type = disabled。方舟深度思考文档（https://docs.volcengine.com/docs/82379/1449737 ，2026-11-05 查）列出的 doubao-seed-* 全部支持 disabled，同平台的别的模型不一定（glm-5-3-flash-260828 只支持 enabled），所以接口地址属于 volces.com 且模型属于这个系列才算。与 client-providers 的 disables_ark_seed_thinking 一致。
   */
  static disablesArkSeedThinking(endpoint: string, model: string): boolean {
    const scheme: number = endpoint.indexOf("://");
    if (scheme < 0) return false;
    let host: string = endpoint.substring(scheme + 3);
    const slash: number = host.indexOf("/");
    if (slash >= 0) host = host.substring(0, slash);
    host = host.substring(host.lastIndexOf("@") + 1);
    const colon: number = host.indexOf(":");
    if (colon >= 0) host = host.substring(0, colon);
    host = host.toLowerCase();
    if (host !== "volces.com" && !host.endsWith(".volces.com")) return false;
    return model.trim().toLowerCase().startsWith("doubao-seed-");
  }
}
