import { useEffect, useState } from "react";
import type {
  MobileKeyboardFeedback,
  MobileKeyboardFeedbackClient,
} from "./mobile-keyboard-feedback-section";

export interface UseMobileKeyboardFeedbackOptions {
  mobile: boolean;
  client?: MobileKeyboardFeedbackClient;
  onError: (message: string) => void;
}

/** Loads and persists native mobile keyboard sound, haptic, and suggestion preferences. */
export function useMobileKeyboardFeedback({
  mobile,
  client,
  onError,
}: UseMobileKeyboardFeedbackOptions) {
  const [value, setValue] = useState<MobileKeyboardFeedback>();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!mobile || !client) {
      setValue(undefined);
      return;
    }
    let active = true;
    void client
      .load()
      .then((next) => {
        if (active) setValue(next);
      })
      .catch(() => {
        if (active) onError("无法读取按键反馈设置，请重试。");
      });
    return () => {
      active = false;
    };
  }, [client, mobile, onError]);

  async function save(next: MobileKeyboardFeedback) {
    if (!client) return;
    const previous = value;
    setValue(next);
    setBusy(true);
    onError("");
    try {
      setValue(await client.save(next));
    } catch {
      if (previous) setValue(previous);
      onError("无法保存按键反馈设置，请重试。");
    } finally {
      setBusy(false);
    }
  }

  async function preview() {
    if (!client?.preview || !value?.hapticsEnabled) return;
    onError("");
    try {
      await client.preview(value.hapticStrength);
    } catch {
      onError("无法预览按键振动，请重试。");
    }
  }

  return { value, busy, save, preview };
}
