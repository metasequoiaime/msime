import { useEffect, useRef, useState } from "react";
import type {
  MobileKeyboardFeedback,
  MobileKeyboardFeedbackClient,
} from "./mobile-keyboard-feedback-section";
import { useMountedRef } from "./use-mounted-ref";

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
  const generation = useRef(0);
  const saveRunning = useRef(false);
  const mounted = useMountedRef();

  useEffect(() => {
    const current = ++generation.current;
    saveRunning.current = false;
    setBusy(false);
    if (!mobile || !client) {
      setValue(undefined);
      return () => {
        if (generation.current === current) generation.current++;
      };
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
    return () => {
      if (generation.current === current) generation.current++;
    };
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
      if (generation.current === current) setValue(saved);
    } catch {
      if (generation.current === current) {
        if (previous) setValue(previous);
        onError("无法保存按键反馈设置，请重试。");
      }
    } finally {
      if (generation.current === current) {
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
      onError("无法预览按键振动，请重试。");
    }
  }

  return { value, busy, save, preview };
}
