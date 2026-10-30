/**
 * 华为账号登录里不碰 Account Kit 和 NAPI 的那一半：读 Rust `huawei_challenge` 的回答、决定授权请求带什么、核对授权响应、把 Account Kit 的错误码换成共享账号页认的 `account_*` 码。ArkTS 一侧（`HarmonyHuaweiSignIn`）只做调用，这里可以在 node 下测试。
 *
 * 依据是华为开发者文档的 `authentication` API 参考（`harmonyos-references/account-api-authentication`）和「ID Token的使用场景与使用方法」（`harmonyos-guides/account-faq-12`）：授权请求的 `scopes` 带 `openid`、`permissions` 带 `idtoken` 时响应里才有 ID Token；`nonce` 只收 `^[0-9a-zA-Z:/\.\-_]{1,255}$`，不传时 ID Token 里是缺省的「default」，服务端一定拒绝；ID Token 的 `sub` 是 UnionID，服务端按它认人。
 */

type JsonObject = Record<string, unknown>;

function jsonObject(value: unknown): value is JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function parseObject(text: string): JsonObject | null {
  try {
    const value: unknown = JSON.parse(text);
    return jsonObject(value) ? value : null;
  } catch {
    return null;
  }
}

/** 只要用户标识，不要头像昵称、手机号这类需要另外申请权限的 scope。 */
export const HUAWEI_ID_SCOPES: readonly string[] = ["openid"];

/** 只要 ID Token；不要授权码（`serviceauthcode`），服务端不换码，也就不需要 Client Secret。 */
export const HUAWEI_ID_PERMISSIONS: readonly string[] = ["idtoken"];

const NONCE_PATTERN: RegExp = /^[0-9a-zA-Z:/.\-_]{1,255}$/;

/** Account Kit 接受的 nonce 形状。服务端给的是 64 位十六进制，总是合格；不合格时不发请求，免得拿到一个带着「default」的令牌。 */
export function validNonce(nonce: string): boolean {
  return NONCE_PATTERN.test(nonce);
}

/** `huawei_challenge` 回答里宿主要用的两项。 */
export interface HuaweiChallenge {
  readonly challengeId: string;
  readonly nonce: string;
}

/** Rust `huawei_challenge` 的回答 `{ok,value:{challengeId,expiresIn,nonce}}`；失败、缺项或 nonce 不合 Account Kit 的要求时为 null，原回答交回页面。 */
export function huaweiChallenge(reply: string): HuaweiChallenge | null {
  const envelope: JsonObject | null = parseObject(reply);
  if (envelope === null || envelope.ok !== true || !jsonObject(envelope.value)) return null;
  const challengeId: unknown = envelope.value.challengeId;
  const nonce: unknown = envelope.value.nonce;
  if (typeof challengeId !== "string" || challengeId.length === 0) return null;
  if (typeof nonce !== "string" || !validNonce(nonce)) return null;
  return { challengeId, nonce };
}

/** 响应的 state 与请求的一致才算本次请求的结果；两边缺省都按空串比，与官方示例相同。 */
export function stateMatches(requested: string | undefined, returned: string | undefined): boolean {
  return (requested ?? "") === (returned ?? "");
}

/** 授权响应里的 ID Token：没有（`permissions` 没带 `idtoken`、Account Kit 没给）或超出文档写的 1–2048 长度时为 null。 */
export function responseIdToken(idToken: string | undefined): string | null {
  return typeof idToken === "string" && idToken.length > 0 && idToken.length <= 2048
    ? idToken
    : null;
}

// Account Kit 的错误码：`authentication.AuthenticationErrorCode` 与系统服务的 `12300001`。
/** 用户未登录华为账号；`forceAuthorization` 为 true 时 Account Kit 会先拉起登录页，一般到不了这里。 */
export const HUAWEI_ID_NOT_SIGNED_IN: number = 1001502001;
export const HUAWEI_ID_NETWORK: number = 1001502005;
export const HUAWEI_ID_USER_CANCELLED: number = 1001502012;
/** 重复请求：上一次授权页还开着。 */
export const HUAWEI_ID_DUPLICATE_REQUEST: number = 1001500002;
/** 应用指纹证书校验失败：AppGallery Connect 的签名或 Client ID 配置不对。 */
export const HUAWEI_ID_FINGERPRINT: number = 1001500001;
/** 应用未申请 scopes 或 permissions 权限。 */
export const HUAWEI_ID_NOT_PERMITTED: number = 1001502014;
export const HUAWEI_ID_SYSTEM_SERVICE: number = 12300001;

/** Account Kit 的错误换成账号页的码：用户关掉授权页和重复请求是安静的取消，网络错误单独提示，其余（包括配置问题）都是「暂不可用」，具体的码只进日志。 */
export function huaweiIdErrorCode(code: number | undefined): string {
  switch (code) {
    case HUAWEI_ID_USER_CANCELLED:
    case HUAWEI_ID_DUPLICATE_REQUEST:
      return "account_cancelled";
    case HUAWEI_ID_NETWORK:
      return "account_network";
    default:
      return "account_unavailable";
  }
}

/** 是不是要在日志里点明「去查 AppGallery Connect 的配置」的错误：指纹证书或 scope 权限不对时重试没有用。 */
export function huaweiIdConfigurationError(code: number | undefined): boolean {
  return code === HUAWEI_ID_FINGERPRINT || code === HUAWEI_ID_NOT_PERMITTED;
}

/** 交回 ID Token 的 `huawei_login` 动作，经宿主的账号通道发给 Rust。 */
export function huaweiLoginAction(challengeId: string, idToken: string): string {
  return JSON.stringify({
    operation: "huawei_login",
    challenge_id: challengeId,
    id_token: idToken,
  });
}

/** 申请 challenge 的 `huawei_challenge` 动作。 */
export function huaweiChallengeAction(): string {
  return JSON.stringify({ operation: "huawei_challenge" });
}
