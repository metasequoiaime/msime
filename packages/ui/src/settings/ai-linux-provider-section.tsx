import * as settings from "./settings-style";

/** Explains where Linux keeps the AI provider credentials: a note in the 服务 group. */
export function AiLinuxProviderSection() {
  return (
    <p className={settings.groupNote}>
      AI 请求由用户管理的 AI 服务完成。凭据不保存在共享设置中；请在用户配置目录的{" "}
      <code>ai-provider.json</code> 中配置，并使其中的 provider、接口地址和模型与本组的设置一致。
    </p>
  );
}
