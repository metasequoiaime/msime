import type {
  ProviderCredentialClient,
  VoiceCredentialKind,
  VoiceProviderCredential,
} from "../core/provider-credential-contracts";
import { ASR_PROVIDER_DEFAULTS, doubaoStreamEndpointId } from "../voice/voice-providers";
import type { CredentialStatusMessageValue } from "./credential-status-message-model";
import type { ProviderCredentialInput } from "./provider-credential-input";

export type VoiceCredentialSectionKind = VoiceCredentialKind;
export type VoiceCredentialEntry = VoiceProviderCredential;
export type VoiceCredentialInput = ProviderCredentialInput;
export type VoiceCredentialSaveInput = Parameters<ProviderCredentialClient["saveVoice"]>[0];
export type VoiceCredentialMessage = CredentialStatusMessageValue;

export interface VoiceCredentialStatus {
  voiceAsr: readonly VoiceCredentialEntry[];
  voicePolish: readonly VoiceCredentialEntry[];
  voiceInvalid: boolean;
}

export interface VoiceCredentialEditorModelOptions {
  kind: VoiceCredentialSectionKind;
  provider: string;
  model: string;
  resourceId?: string;
  authMode?: string;
  credentials?: VoiceCredentialStatus;
  input: VoiceCredentialInput;
  busy: boolean;
}

export interface VoiceCredentialEditorModel {
  doubao: boolean;
  legacy: boolean;
  stored: VoiceCredentialEntry | undefined;
  endpoint: string;
  streamEndpoint: string;
  customEndpoint: boolean;
  name: string;
  tokenLabel: string;
  /** 凭据现在的状态（未保存、已保存、与上方设置不一致、文件无效），常驻标题下方。 */
  status: string;
  /** 凭据存在哪里、谁来读：解释，收进标题旁的「?」。 */
  description: string;
  saveDisabled: boolean;
  clearDisabled: boolean;
}

/** 凭据编辑的回退、绑定提示与写入条件；不读取文件或回填已保存密钥。 */
export function voiceCredentialEditorModel({
  kind,
  provider,
  model,
  resourceId,
  authMode,
  credentials,
  input,
  busy,
}: VoiceCredentialEditorModelOptions): VoiceCredentialEditorModel {
  const doubao = kind === "asr" && provider === "doubao";
  const legacy = doubao && authMode === "legacy";
  const stored = (kind === "asr" ? credentials?.voiceAsr : credentials?.voicePolish)?.find(
    (entry) => entry.provider === provider,
  );
  const endpoint = input.endpoint ?? stored?.endpoint ?? "";
  const name = kind === "asr" ? "识别" : "润色";
  const mismatch =
    stored &&
    ((model.trim() && stored.model !== model.trim()) ||
      (doubao &&
        ((resourceId?.trim() && stored.resourceId !== resourceId.trim()) ||
          stored.authMode !== authMode)));
  const streamEndpoint = endpoint.trim() || ASR_PROVIDER_DEFAULTS.doubao.endpoint;
  return {
    doubao,
    legacy,
    stored,
    endpoint,
    streamEndpoint,
    customEndpoint: doubaoStreamEndpointId(streamEndpoint) === "custom",
    name,
    tokenLabel: doubao && !legacy ? "Doubao API Key" : `${name} API Token`,
    status: credentials?.voiceInvalid
      ? "现有 voice-provider.json 无效，语音服务不会启动；请修复或删除该文件"
      : !stored
        ? "尚未保存"
        : mismatch
          ? "已保存的凭据与上方模型或豆包设置不一致；保存后改为绑定当前设置"
          : "已保存，留空则保留原凭据",
    description: "保存后只写入用户配置目录的 voice-provider.json，由语音服务读取。",
    saveDisabled:
      busy ||
      (!input.token.trim() && !stored) ||
      (legacy && !input.appKey.trim() && stored?.authMode !== "legacy"),
    clearDisabled: busy,
  };
}

/** 空白密钥不提交；非空密钥沿用原文，由宿主保存时完成原有校验。 */
export function voiceCredentialSavePayload(
  options: VoiceCredentialEditorModelOptions,
): VoiceCredentialSaveInput {
  const { kind, provider, model, resourceId, authMode, input } = options;
  const { doubao, legacy, endpoint } = voiceCredentialEditorModel(options);
  return {
    kind,
    provider,
    endpoint,
    model,
    ...(input.token.trim() ? { token: input.token } : {}),
    ...(legacy && input.appKey.trim() ? { appKey: input.appKey } : {}),
    resourceId: doubao ? (resourceId ?? "") : "",
    authMode: doubao ? (authMode ?? "") : "",
  };
}
