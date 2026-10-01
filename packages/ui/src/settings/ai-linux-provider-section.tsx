import type { ReactNode } from "react";
import * as settings from "./settings-style";

export interface AiLinuxProviderSectionProps {
  children?: ReactNode;
  /** 放进 AI 辅助页的「服务」组时为真：只留一段组内说明，不再画自己的卡片和标题。 */
  grouped?: boolean;
}

/** Explains where Linux keeps the AI provider credentials and exposes optional test controls. */
export function AiLinuxProviderSection({ children, grouped = false }: AiLinuxProviderSectionProps) {
  const note = (
    <>
      凭据不保存在共享设置中；请在用户配置目录的 <code>ai-provider.json</code> 中配置，并使其中的
      provider、接口地址和模型与{grouped ? "本组的" : "上方"}设置一致。
    </>
  );
  if (grouped) {
    return (
      <>
        <p className={settings.groupNote}>AI 请求由用户管理的 AI 服务完成。{note}</p>
        {children}
      </>
    );
  }
  return (
    <div className="section">
      <div className="section-title">
        Linux AI provider<small>AI 请求由用户管理的 provider 服务完成</small>
      </div>
      <p className="input-setting-description">{note}</p>
      {children}
    </div>
  );
}
