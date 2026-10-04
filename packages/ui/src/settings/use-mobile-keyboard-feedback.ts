import { useEffect, useRef, useState } from "react";
import type {
  MobileKeyboardFeedback,
  MobileKeyboardFeedbackClient,
} from "./mobile-keyboard-feedback-section";
import { useMountedRef } from "./use-mounted-ref";
import { useAsyncGeneration } from "./use-async-generation";

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
  const saveRunning = useRef(false);
  const mounted = useMountedRef();
  const generation = useAsyncGeneration(client, mobile, onError);

  useEffect(() => {
    const current = generation.current;
    saveRunning.current = false;
    setBusy(false);
    if (!mobile || !client) {
      setValue(undefined);
      return;
    }
    void client
      .load()
      .then((next) => {
        if (mounted.current && generation.current === current) setValue(next);
      })
      .catch(() => {
        if (mounted.current && generation.current === current)
          onError("无法读取按键反馈设置，请重试。");
      });
  }, [client, mobile, onError, mounted]);

  async function save(next: MobileKeyboardFeedback) {
    if (!client || saveRunning.current) return;
    const current = generation.current;
    const previous = value;
    saveRunning.current = true;
    setValue(next);
    setBusy(true);
    onError("");
    try {
      const saved = await client.save(next);
      if (mounted.current && generation.current === current) setValue(saved);
    } catch {
      if (mounted.current && generation.current === current) {
        if (previous) setValue(previous);
        onError("无法保存按键反馈设置，请重试。");
      }
    } finally {
      if (mounted.current && generation.current === current) {
        saveRunning.current = false;
        setBusy(false);
      }
    }
  }

  async function preview() {
    if (!client?.preview || !value?.hapticsEnabled) return;
    onError("");
    try {
      await client.preview(value.hapticStrength);
    } catch {
      if (mounted.current) onError("无法预览按键振动，请重试。");
    }
  }

  return { value, busy, save, preview };
}
