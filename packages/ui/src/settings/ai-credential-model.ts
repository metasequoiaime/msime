import type {
  ProviderCredentialClient,
  ProviderCredentialStatus,
} from "../core/provider-credential-contracts";
import type { AiCredentialPreferences } from "./ai-credential-preferences";

export type AiCredentialStored = Pick<ProviderCredentialStatus["ai"][number], "endpoint" | "model">;
export type AiCredentialSaveInput = Parameters<ProviderCredentialClient["saveAi"]>[0];

export interface AiCredentialEditorModelOptions extends Pick<
  AiCredentialPreferences,
  "endpoint" | "model"
> {
  origin: string | null;
  token: string;
  stored?: AiCredentialStored;
  invalid: boolean;
  busy: boolean;
}

export interface AiCredentialEditorModel {
  matchesStored: boolean;
  hasStoredCredential: boolean;
  description: string;
  tokenDisabled: boolean;
  saveDisabled: boolean;
  clearDisabled: boolean;
}

/** AI 凭据的精确绑定、状态说明与编辑条件；接口来源由调用方解析。 */
export function aiCredentialEditorModel({
  endpoint,
  model,
  origin,
  token,
  stored,
  invalid,
  busy,
}: AiCredentialEditorModelOptions): AiCredentialEditorModel {
  const matchesStored = stored?.endpoint === endpoint && stored.model === model;
  return {
    matchesStored,
    hasStoredCredential: Boolean(stored),
    description: invalid
      ? "现有 ai-provider.json 无效，provider 服务不会发出任何 AI 请求；请修复或删除该文件"
      : !stored
        ? "尚未保存；保存后只写入用户配置目录的 ai-provider.json，由 provider 服务读取"
        : matchesStored
          ? "已保存，留空则保留原凭据"
          : `已保存的凭据绑定 ${stored.endpoint}（${stored.model}），与上方设置不一致；保存后改为绑定当前接口和模型`,
    tokenDisabled: !origin,
    saveDisabled: busy || !origin || !model.trim() || (!token.trim() && !stored),
    clearDisabled: busy,
  };
}
