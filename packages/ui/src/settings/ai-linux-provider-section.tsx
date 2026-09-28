import type { ReactNode } from "react";

export interface AiLinuxProviderSectionProps {
  children?: ReactNode;
}

/** Explains where Linux keeps the AI provider credentials and exposes optional test controls. */
export function AiLinuxProviderSection({ children }: AiLinuxProviderSectionProps) {
  return (
    <div className="section">
      <div className="section-title">
        Linux AI provider<small>AI 请求由用户管理的 provider 服务完成</small>
      </div>
      <p className="input-setting-description">
        凭据不保存在共享设置中；请在用户配置目录的 <code>ai-provider.json</code> 中配置，并使其中的
        provider、接口地址和模型与上方设置一致。
      </p>
      {children}
    </div>
  );
}
