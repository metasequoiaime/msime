import { useCallback, useEffect, useRef, useState } from "react";
import { useLatestRef } from "../core/use-latest-ref";
import type {
  MobileKeyboardFeedback,
  MobileKeyboardFeedbackClient,
} from "./mobile-keyboard-feedback";
import { useAsyncActionRunner } from "../core/use-async-action";

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
  const onErrorRef = useLatestRef(onError);
  const previewRequest = useRef(0);
  const ignoreError = useCallback(() => {}, []);
  const {
    busy,
    mounted,
    generation,
    running: saveRunning,
    run,
  } = useAsyncActionRunner(ignoreError, undefined, client, mobile);

  useEffect(() => {
    if (!mobile || !client) {
      setValue(undefined);
      return;
    }
    setValue(undefined);
    void run(
      async (isCurrent) => {
        const next = await client.load();
        if (isCurrent()) setValue(next);
      },
      {
        formatError: () => "无法读取按键反馈设置，请重试。",
        onError: () => onErrorRef.current("无法读取按键反馈设置，请重试。"),
      },
    );
  }, [client, mobile, run]);

  async function save(next: MobileKeyboardFeedback) {
    if (!client || saveRunning.current) return;
    previewRequest.current += 1;
    const previous = value;
    onErrorRef.current("");
    await run(
      async (isCurrent) => {
        setValue(next);
        const saved = await client.save(next);
        if (isCurrent()) setValue(saved);
      },
      {
        formatError: () => "无法保存按键反馈设置，请重试。",
        onError: () => {
          if (previous) setValue(previous);
          onErrorRef.current("无法保存按键反馈设置，请重试。");
        },
      },
    );
  }

  async function preview() {
    if (!client?.preview || !value?.hapticsEnabled) return;
    const owner = generation.current;
    const request = ++previewRequest.current;
    onErrorRef.current("");
    try {
      await client.preview(value.hapticStrength);
    } catch {
      if (mounted.current && generation.current === owner && previewRequest.current === request)
        onErrorRef.current("无法预览按键振动，请重试。");
    }
  }

  return { value, busy, save, preview };
}
