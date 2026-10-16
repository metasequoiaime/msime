/** Linux 在线服务持有的凭据状态，不含秘密本身。 */
export type ProviderCredentialStatus = {
  ai: { provider: string; endpoint: string; model: string }[];
  /** AI 凭据文件存在但被服务拒绝，修复前没有可用的 AI 服务。 */
  aiInvalid: boolean;
  tencent: { region: string } | null;
  tencentInvalid: boolean;
  voiceAsr: VoiceProviderCredential[];
  voicePolish: VoiceProviderCredential[];
  /** 语音凭据文件存在但被服务拒绝，修复前语音输入无法启动。 */
  voiceInvalid: boolean;
};
/** 已保存的语音凭据条目；空 `model` / `endpoint` 表示使用服务商默认值。 */
export type VoiceProviderCredential = {
  provider: string;
  model: string;
  endpoint: string;
  resourceId: string | null;
  authMode: string | null;
};
export type VoiceCredentialKind = "asr" | "polish";
export type VoiceCredentialSaveResult = {
  status: ProviderCredentialStatus;
  /** 用户服务管理器是否接受语音套接字变更；无论结果如何，凭据文件都已保存。 */
  serviceUpdated: boolean;
};
/**
 * Linux 宿主为在线服务维护的仅所有者可访问凭据。秘密传入 undefined 时保留已存值，因此只改端点、模型或地域不需要重新输入密钥。
 */
export type ProviderCredentialClient = {
  status(): Promise<ProviderCredentialStatus>;
  saveAi(credential: {
    provider: string;
    endpoint: string;
    model: string;
    token?: string;
  }): Promise<ProviderCredentialStatus>;
  clearAi(provider: string): Promise<ProviderCredentialStatus>;
  saveTencent(credential: {
    secretId?: string;
    secretKey?: string;
    region: string;
  }): Promise<ProviderCredentialStatus>;
  clearTencent(): Promise<ProviderCredentialStatus>;
  saveVoice(credential: {
    kind: VoiceCredentialKind;
    provider: string;
    endpoint: string;
    model: string;
    token?: string;
    appKey?: string;
    resourceId: string;
    authMode: string;
  }): Promise<VoiceCredentialSaveResult>;
  clearVoice(kind: VoiceCredentialKind, provider: string): Promise<VoiceCredentialSaveResult>;
};
