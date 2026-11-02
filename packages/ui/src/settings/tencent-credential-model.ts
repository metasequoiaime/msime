import type {
  ProviderCredentialClient,
  ProviderCredentialStatus,
} from "../core/provider-credential-contracts";
import { tencentCredentialIssue } from "./translation-validation";

/** 腾讯云 provider 凭据编辑的临时输入；空白密钥表示沿用已存值。 */
export interface TencentCredentialInput {
  secretId: string;
  secretKey: string;
  region?: string;
}

export type TencentCredentialStatus = Pick<ProviderCredentialStatus, "tencent" | "tencentInvalid">;
export type TencentCredentialSaveInput = Parameters<ProviderCredentialClient["saveTencent"]>[0];

export interface TencentCredentialSaveOptions {
  status?: TencentCredentialStatus;
  input: TencentCredentialInput;
}

export interface TencentCredentialEditorModelOptions extends TencentCredentialSaveOptions {
  busy: boolean;
}

export interface TencentCredentialEditorModel {
  stored: TencentCredentialStatus["tencent"] | undefined;
  region: string;
  /** 凭据现在的状态（文件无效、已保存），常驻；还没保存时为 null。 */
  status: string | null;
  /** 凭据存在哪里、谁来读：解释，收进「在线翻译服务」旁的「?」。 */
  description: string;
  regionInvalid: boolean;
  saveDisabled: boolean;
  clearDisabled: boolean;
}

function tencentCredentialRegion({ status, input }: TencentCredentialSaveOptions): string {
  return input.region ?? status?.tencent?.region ?? "ap-guangzhou";
}

/** 地域回退、状态说明与操作条件；地域格式只影响默认展开，保存仍交由宿主校验。 */
export function tencentCredentialEditorModel(
  options: TencentCredentialEditorModelOptions,
): TencentCredentialEditorModel {
  const { input, status, busy } = options;
  const stored = status?.tencent;
  const region = tencentCredentialRegion(options);
  return {
    stored,
    region,
    status: status?.tencentInvalid
      ? "现有 tencent-provider.json 无效，provider 服务不会发出翻译请求；请修复或删除该文件。"
      : stored
        ? "腾讯云凭据已保存；SecretId 和 SecretKey 留空则保留原值。"
        : null,
    description:
      "由用户管理的 Linux provider 服务负责网络请求和凭据。凭据只写入用户配置目录的 tencent-provider.json，由 provider 服务读取，不进入共享设置。",
    regionInvalid: tencentCredentialIssue("", "", region) !== "",
    saveDisabled: busy || (!stored && (!input.secretId.trim() || !input.secretKey.trim())),
    clearDisabled: busy,
  };
}

/** 空白密钥省略提交，非空密钥和显式地域保留原文。 */
export function tencentCredentialSavePayload(
  options: TencentCredentialSaveOptions,
): TencentCredentialSaveInput {
  const { input } = options;
  return {
    ...(input.secretId.trim() ? { secretId: input.secretId } : {}),
    ...(input.secretKey.trim() ? { secretKey: input.secretKey } : {}),
    region: tencentCredentialRegion(options),
  };
}
