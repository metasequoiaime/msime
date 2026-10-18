import { useEffect, useRef, useState } from "react";
import { trackAsyncSubscription } from "../core/async-subscription";
import { utf8ByteLength } from "../core/text";
import { subscribeToWindowEvent } from "../core/window-focus";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { usePanelDrag } from "./use-panel-drag";
import { validVoiceLanguage } from "./voice-panel";
import type { VoicePanelClient } from "./panel-client";

/** 语音面板的录音、订阅、文本修订与提交会话；界面继续决定控件外观和禁用条件。 */
export function useVoicePanel(client: VoicePanelClient) {
  const [inputLevel, setInputLevel] = useState<number | undefined>();
  const [language, setLanguage] = useState("zh-CN");
  const [text, setText] = useState("");
  const exceedsSubmitLimit =
    client.maxSubmitBytes !== undefined && utf8ByteLength(text) > client.maxSubmitBytes;
  const textRevision = useAsyncGeneration();
  function updateText(value: string) {
    textRevision.current++;
    setText(value);
  }
  const [submitting, setSubmitting] = useState(false);
  const [copying, setCopying] = useState(false);
  const submittingRef = useRef(false);
  const submissionRevision = useAsyncGeneration();
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [stopping, setStopping] = useState(false);
  const stoppingRef = useRef(false);
  const recognitionRevision = useAsyncGeneration();
  const [notice, setNotice] = useState("点击开始后由宿主录音并进行语音识别");
  const drag = usePanelDrag(client, () => setNotice("无法移动窗口，请重试。"));
  const voiceGeneration = useAsyncGeneration(client);

  useEffect(() => {
    const generation = voiceGeneration.current;
    if (!client.loadVoiceLanguage) return;
    void client
      .loadVoiceLanguage()
      .then((next) => {
        if (generation === voiceGeneration.current && validVoiceLanguage(next)) setLanguage(next);
      })
      .catch(() => undefined);
  }, [client, voiceGeneration]);

  useEffect(() => {
    const generation = voiceGeneration.current;
    if (!client.rememberInputTarget) return;
    void client.rememberInputTarget().catch(() => {
      if (generation === voiceGeneration.current) setNotice("未能记录前台输入窗口");
    });
  }, [client, voiceGeneration]);

  useEffect(() => {
    submittingRef.current = false;
    setSubmitting(false);
    setCopying(false);
    setBusy(false);
    stoppingRef.current = false;
    setStopping(false);
    return () => {
      submissionRevision.current++;
      submittingRef.current = false;
      recognitionRevision.current++;
      const wasBusy = busyRef.current;
      busyRef.current = false;
      if (wasBusy && client.cancelVoice) void client.cancelVoice().catch(() => undefined);
    };
  }, [client]);

  useEffect(() => {
    if (!client.onVoiceUpdate) return;
    const generation = voiceGeneration.current;
    return trackAsyncSubscription(
      client.onVoiceUpdate((update) => {
        if (generation !== voiceGeneration.current || !busyRef.current) return;
        if (update.level !== undefined) {
          if (
            !stoppingRef.current &&
            Number.isFinite(update.level) &&
            update.level >= 0 &&
            update.level <= 1
          ) {
            setInputLevel(update.level);
          }
          return;
        }
        if (update.phase) {
          const labels = {
            recording: "正在录音…",
            recognizing: "正在识别…",
            polishing: "正在润色…",
          };
          setNotice(labels[update.phase]);
          if (update.phase !== "recording") {
            stoppingRef.current = true;
            setStopping(true);
          }
          return;
        }
        updateText(update.text);
        setNotice(
          update.final
            ? update.text
              ? "识别完成，点击提交即可输入"
              : "没有识别到内容"
            : stoppingRef.current
              ? "正在识别…"
              : "正在录音并识别…",
        );
      }),
      () => generation === voiceGeneration.current,
    );
  }, [client, voiceGeneration]);

  async function recognize() {
    if (busyRef.current || submittingRef.current) return;
    if (!client.recognizeVoice) {
      setNotice("当前宿主未提供语音识别能力");
      return;
    }
    if (!validVoiceLanguage(language)) {
      setNotice("请输入有效的识别语言码");
      return;
    }
    const revision = ++recognitionRevision.current;
    busyRef.current = true;
    setBusy(true);
    setInputLevel(undefined);
    updateText("");
    setNotice("正在录音并识别…");
    try {
      const result = await client.recognizeVoice(language);
      if (revision !== recognitionRevision.current) return;
      updateText(result.text);
      setNotice(result.text ? "识别完成，点击提交即可输入" : "没有识别到内容");
    } catch {
      if (revision === recognitionRevision.current) setNotice("语音识别失败，请确认录音服务已启动");
    } finally {
      if (revision === recognitionRevision.current) {
        busyRef.current = false;
        setBusy(false);
        stoppingRef.current = false;
        setStopping(false);
      }
    }
  }

  async function submit(copyOnly = false) {
    const send = copyOnly ? client.copyText : (client.sendVoiceText ?? client.sendText);
    if (!text || !send || busyRef.current || submittingRef.current) return;
    if (!copyOnly && exceedsSubmitLimit) {
      setNotice("内容超过单次提交长度，请精简或复制结果后手动粘贴");
      return;
    }
    const revision = ++submissionRevision.current;
    const submittedTextRevision = textRevision.current;
    submittingRef.current = true;
    setSubmitting(true);
    setCopying(copyOnly);
    setNotice(copyOnly ? "正在复制…" : "正在提交…");
    try {
      await send(text);
      if (revision !== submissionRevision.current) return;
      if (submittedTextRevision !== textRevision.current) {
        setNotice(
          copyOnly ? "上一版内容已复制，当前内容已保留" : "上一版内容已提交，当前内容已保留",
        );
        return;
      }
      if (copyOnly) {
        setNotice("识别结果已复制，文本已保留");
      } else {
        setNotice(client.submitNotice ?? `已提交：${text}`);
        updateText("");
      }
    } catch {
      if (revision === submissionRevision.current)
        setNotice(copyOnly ? "复制失败，识别结果已保留" : "提交失败，前台输入窗口可能已关闭");
    } finally {
      if (revision === submissionRevision.current) {
        submittingRef.current = false;
        setSubmitting(false);
        setCopying(false);
      }
    }
  }

  async function stop() {
    if (!client.stopVoice) {
      await cancel();
      return;
    }
    if (!busyRef.current || stoppingRef.current) return;
    const revision = recognitionRevision.current;
    stoppingRef.current = true;
    setStopping(true);
    setNotice("正在完成识别…");
    try {
      await client.stopVoice();
    } catch {
      if (revision === recognitionRevision.current && busyRef.current) {
        stoppingRef.current = false;
        setStopping(false);
        setNotice("停止录音失败，请重试或取消录音");
      }
    }
  }

  async function cancel() {
    recognitionRevision.current++;
    busyRef.current = false;
    // 即使宿主确认延迟，本地取消也立即生效。
    setBusy(false);
    stoppingRef.current = false;
    setStopping(false);
    setNotice("录音已停止");
    try {
      await client.cancelVoice?.();
    } catch {
      /* 宿主可能已经停止录音。 */
    }
  }

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.key !== "Escape" ||
        event.isComposing ||
        event.ctrlKey ||
        event.altKey ||
        event.metaKey ||
        event.shiftKey ||
        !busyRef.current ||
        !client.cancelVoice
      )
        return;
      event.preventDefault();
      event.stopPropagation();
      void cancel();
    };
    return subscribeToWindowEvent("keydown", onKeyDown);
  }, [client]);

  async function close() {
    submissionRevision.current++;
    recognitionRevision.current++;
    const wasBusy = busyRef.current;
    busyRef.current = false;
    setBusy(false);
    stoppingRef.current = false;
    setStopping(false);
    if (wasBusy && client.cancelVoice) {
      try {
        await client.cancelVoice();
      } catch {
        /* 宿主服务退出后仍继续关闭面板。 */
      }
    }
    await client.close();
  }

  function clear() {
    updateText("");
    setNotice("识别结果已清空");
  }

  return {
    inputLevel,
    language,
    setLanguage,
    text,
    exceedsSubmitLimit,
    updateText,
    submitting,
    copying,
    busy,
    stopping,
    notice,
    drag,
    recognize,
    submit,
    stop,
    cancel,
    close,
    clear,
  };
}

export type VoicePanelState = ReturnType<typeof useVoicePanel>;
