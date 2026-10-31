import { useEffect, useState } from "react";
import { SwitchRow } from "./switch-row";
import { useToast } from "../core/toast";
import type { ClipboardDeleteAuthenticationClient } from "../core/host-contracts";

/** 宿主拒绝切换的错误码：设备没有可用的系统认证（没设锁屏密码），打开以后会再也关不掉，所以一律不许切换。 */
export const CLIPBOARD_DELETE_AUTHENTICATION_UNAVAILABLE =
  "clipboard_delete_authentication_unavailable";
/** 宿主拒绝切换的错误码：系统认证被取消或没有通过。 */
export const CLIPBOARD_DELETE_AUTHENTICATION_FAILED = "clipboard_delete_authentication_failed";
/** 宿主拒绝切换的错误码：上一次切换的认证还没结束。 */
export const CLIPBOARD_DELETE_AUTHENTICATION_BUSY = "clipboard_delete_authentication_busy";

/** 切换没成功时给用户看的话；措辞与 Android 键盘页相同。 */
export function clipboardDeleteAuthenticationRefusal(error: unknown, next: boolean): string {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? (error as { code: unknown }).code
      : undefined;
  if (code === CLIPBOARD_DELETE_AUTHENTICATION_UNAVAILABLE)
    return "请先在系统设置中开启锁屏验证（指纹、图案、密码等）后再使用";
  if (code === CLIPBOARD_DELETE_AUTHENTICATION_FAILED)
    return next
      ? "未通过系统认证，剪贴板删除数据验证保持关闭"
      : "未通过系统认证，剪贴板删除数据验证保持开启";
  if (code === CLIPBOARD_DELETE_AUTHENTICATION_BUSY) return "正在验证身份，请先完成上一次验证";
  return "没有保存，请重试";
}

/**
 * 「剪贴板」组里的「剪贴板删除数据验证」（#10408）：宿主提供 `clipboardDeleteAuthentication` 时才画（目前只有 HarmonyOS；Android 和 iOS 的同一个开关在各自的原生页）。开关在宿主答复之前保持原值，被拒绝或没保存时不动，并说明原因。
 */
export function ClipboardDeleteAuthenticationRow({
  client,
}: {
  client: ClipboardDeleteAuthenticationClient;
}) {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const toast = useToast();
  useEffect(() => {
    let current = true;
    setEnabled(null);
    setError("");
    void client.load().then(
      (value) => {
        if (current) setEnabled(value);
      },
      () => {
        if (current) setError("剪贴板删除数据验证的设置读取失败，请重新打开此页。");
      },
    );
    return () => {
      current = false;
    };
  }, [client]);
  async function change(next: boolean) {
    setBusy(true);
    try {
      setEnabled(await client.save(next));
    } catch (refusal) {
      toast(clipboardDeleteAuthenticationRefusal(refusal, next));
    } finally {
      setBusy(false);
    }
  }
  return (
    <SwitchRow
      title="剪贴板删除数据验证"
      aria-label="剪贴板删除数据验证"
      description={
        error ||
        "开启后，在键盘剪贴板里删除或清空记录都要先通过锁屏密码、指纹或人脸等系统认证；打开和关闭本开关也要认证。"
      }
      checked={enabled ?? false}
      disabled={enabled === null || busy}
      onChange={(value) => {
        void change(value);
      }}
    />
  );
}
