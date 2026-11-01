import { useEffect, useState } from "react";
import { GroupList, Row, Switch } from "../core/platform-controls";
import { useToast } from "../core/toast";
import type { ScreenCaptureProtectionClient } from "../core/host-contracts";

/** 宿主拒绝切换的错误码：设备没有可用的系统认证（没设锁屏密码），打开以后会再也关不掉，所以一律不许切换。 */
export const SCREEN_CAPTURE_AUTHENTICATION_UNAVAILABLE =
  "screen_capture_authentication_unavailable";
/** 宿主拒绝切换的错误码：系统认证被取消或没有通过。 */
export const SCREEN_CAPTURE_AUTHENTICATION_FAILED = "screen_capture_authentication_failed";

/** 一处切换成功后通知同一页面里的其他开关：鸿蒙手机上「我的 › 隐私」和「关于 › 许可与隐私」各有一个，常驻的那个不能停在旧值上。 */
const changedEvent = "msime-screen-capture-protection-changed";

/** 切换没成功时给用户看的话；措辞与 Android 隐私页相同。 */
export function screenCaptureProtectionRefusal(error: unknown, next: boolean): string {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? (error as { code: unknown }).code
      : undefined;
  if (code === SCREEN_CAPTURE_AUTHENTICATION_UNAVAILABLE)
    return next
      ? "这台设备没有可用的系统认证，设置锁屏密码后才能打开防止截屏"
      : "这台设备没有可用的系统认证，防止截屏保持开启";
  if (code === SCREEN_CAPTURE_AUTHENTICATION_FAILED)
    return next ? "未通过系统认证，防止截屏保持关闭" : "未通过系统认证，防止截屏保持开启";
  return "没有保存，请重试";
}

/**
 * 「隐私」页的「截屏与录屏」分组：宿主提供 `screenCaptureProtection` 时才画（目前只有 HarmonyOS；Android 的同一个开关在原生隐私页）。开关在宿主答复之前保持原值，被拒绝或没保存时不动，并说明原因。
 *
 * 鸿蒙手机上「我的 › 隐私」和「关于 › 许可与隐私」都画它（2in1 没有前一页），两处经 `changedEvent` 保持一致；宿主按真实状态回答，所以即使某一处停在旧值，切换也不会做错事。
 *
 * 外层带 `data-settings-search-label`：设置搜索只按标题匹配，这样搜「录屏」也能找到这里，焦点落在开关上。
 */
export function ScreenCaptureProtectionGroup({
  client,
}: {
  client: ScreenCaptureProtectionClient | undefined;
}) {
  if (!client) return null;
  return (
    <div className="contents" data-settings-search-label="截屏与录屏">
      <GroupList title="截屏与录屏">
        <ScreenCaptureProtectionRow client={client} />
      </GroupList>
    </div>
  );
}

function ScreenCaptureProtectionRow({ client }: { client: ScreenCaptureProtectionClient }) {
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
        if (current) setError("防止截屏的设置读取失败，请重新打开此页。");
      },
    );
    const changed = (event: Event) => {
      if (current) setEnabled((event as CustomEvent<boolean>).detail);
    };
    window.addEventListener(changedEvent, changed);
    return () => {
      current = false;
      window.removeEventListener(changedEvent, changed);
    };
  }, [client]);
  async function change(next: boolean) {
    setBusy(true);
    try {
      const saved = await client.save(next);
      setEnabled(saved);
      window.dispatchEvent(new CustomEvent<boolean>(changedEvent, { detail: saved }));
    } catch (refusal) {
      toast(screenCaptureProtectionRefusal(refusal, next));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Row
      title="防止截屏"
      description="打开后，设置页面和弹出的键盘在截屏、录屏里显示为黑色。第一次打开不需要验证；打开过以后，再打开或关闭都要通过锁屏密码或生物识别验证。"
      detail={error || undefined}
    >
      <Switch
        checked={enabled ?? false}
        disabled={enabled === null || busy}
        onChange={(value) => {
          void change(value);
        }}
      />
    </Row>
  );
}
