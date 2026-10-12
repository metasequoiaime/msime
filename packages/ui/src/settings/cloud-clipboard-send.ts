import { errorCode } from "../core/error-code";
import type { CloudClipboardAction, CloudClipboardResponse } from "../keyboard/panels";

/** One cloud clipboard request, the same actions the 云剪贴板 panel sends, issued by a host for the settings window itself. */
export type CloudClipboardRequest = (
  action: CloudClipboardAction,
) => Promise<CloudClipboardResponse>;

/** The longest text the cloud clipboard stores, counted in UTF-16 code units the way the server counts it. */
export const CLOUD_CLIPBOARD_MAX_UTF16 = 4000;
export const CLOUD_CLIPBOARD_SIGNED_OUT = "登录水杉账号后可在设备间同步剪贴板";
export const CLOUD_CLIPBOARD_DISABLED = "云剪贴板未开启";
export const CLOUD_CLIPBOARD_UNAVAILABLE = "云剪贴板服务暂不可用，请稍后重试";
/** 自建中继以 401 拒绝了令牌。中继没有登录这回事，要用户在设置里换一个令牌。 */
export const CLOUD_CLIPBOARD_RELAY_UNAUTHORIZED =
  "自建中继拒绝了令牌，请在设置的「剪贴板」页检查中继令牌";
export const CLOUD_CLIPBOARD_PENDING =
  "本设备还没有获准使用云剪贴板：请打开云剪贴板查看本设备的核对码，再到一台已在使用云剪贴板的设备上点「允许」";

/** Whether this window can send to the cloud clipboard right now: `ready` only when the account is signed in (or a relay is configured), the server reports the clipboard enabled and this device holds the clipboard key. `pending` means this device is waiting for another one to approve it; `relay_unauthorized` means the self-hosted relay refused its token. */
export type CloudClipboardAvailability =
  | "checking"
  | "ready"
  | "signed_out"
  | "relay_unauthorized"
  | "disabled"
  | "pending"
  | "unavailable";

/** Reads a failed request: an unauthorized account means signed out (or a session that has expired), a refused relay token is its own case because signing in would not help, the end-to-end codes name a device still waiting for approval or a clipboard switched off, anything else means the service could not be reached. */
export function cloudClipboardFailure(
  error: unknown,
): "signed_out" | "relay_unauthorized" | "pending" | "disabled" | "unavailable" {
  const code = errorCode(error);
  // 必须在下面按 `_unauthorized` 归为未登录之前认出来：中继的令牌被拒和水杉账号无关。
  if (code === "relay_unauthorized") return "relay_unauthorized";
  if (code === "unauthorized" || code?.endsWith("_unauthorized")) return "signed_out";
  if (code === "clipboard_device_pending") return "pending";
  if (code === "clipboard_sync_disabled") return "disabled";
  return "unavailable";
}

/** 云剪贴板请求失败时给用户看的一句话：驱动给出的错误码各有说明，其余用调用方的 `fallback`。 */
export function cloudClipboardErrorMessage(error: unknown, fallback: string): string {
  switch (errorCode(error)) {
    case "clipboard_device_pending":
      return CLOUD_CLIPBOARD_PENDING;
    case "clipboard_sync_disabled":
      return CLOUD_CLIPBOARD_DISABLED;
    case "clipboard_key_conflict":
      return "另一台设备刚刚更换了云剪贴板密钥，请刷新后重试";
    case "clipboard_device_limit":
      return "这个账号登记的设备已达上限，请先移除不再使用的设备";
    case "not_found":
      return "这台设备已不在列表里，请刷新后再试";
    case "account_unauthorized":
    case "unauthorized":
      return CLOUD_CLIPBOARD_SIGNED_OUT;
    case "relay_unauthorized":
      return CLOUD_CLIPBOARD_RELAY_UNAUTHORIZED;
    case "relay_not_configured":
      return "自建中继的地址或令牌无效，请在设置的「剪贴板」页重新填写";
    case "relay_unavailable":
      return "连不上自建中继，请确认中继在运行、地址正确，稍后重试";
    case "relay_storage":
      return "无法读写本机保存的中继密钥，请检查数据目录的权限";
  }
  return fallback;
}

/** The note shown for an availability that blocks sending; `undefined` when there is nothing to explain. */
export function cloudClipboardAvailabilityNote(
  availability: CloudClipboardAvailability,
): string | undefined {
  switch (availability) {
    case "signed_out":
      return CLOUD_CLIPBOARD_SIGNED_OUT;
    case "relay_unauthorized":
      return CLOUD_CLIPBOARD_RELAY_UNAUTHORIZED;
    case "disabled":
      return CLOUD_CLIPBOARD_DISABLED;
    case "pending":
      return CLOUD_CLIPBOARD_PENDING;
    case "unavailable":
      return CLOUD_CLIPBOARD_UNAVAILABLE;
  }
  return undefined;
}
