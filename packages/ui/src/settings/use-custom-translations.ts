import { useEffect, useMemo, useRef, useState } from "react";
import { errorMessage } from "../core/error-message";
import {
  customTranslationsExample,
  customTranslationsWithinBounds,
  parseCustomTranslations,
} from "../dictionary/custom-translations";
import {
  SETTINGS_AUTOSAVE_DELAY_MS,
  SETTINGS_SAVED_STATUS_MS,
  type SettingsSaveState,
} from "./use-settings-persistence";
import { useFlushOnWindowLeave } from "./use-flush-on-window-leave";
import { useMountedRef } from "./use-mounted-ref";

export interface CustomTranslationsClient {
  load(): Promise<string>;
  save(text: string): Promise<void>;
}

export interface UseCustomTranslationsOptions {
  client?: CustomTranslationsClient;
}

const oversizedNotice = "自定义释义过大，请精简后再保存。";

/** Owns loading, validation, automatic saving, and summary text for user translation overlays. */
export function useCustomTranslations({ client }: UseCustomTranslationsOptions) {
  const [text, setTextState] = useState("");
  const [notice, setNotice] = useState("");
  const [saveState, setSaveState] = useState<SettingsSaveState>("idle");
  const [saveError, setSaveError] = useState("");
  const report = useMemo(() => parseCustomTranslations(text), [text]);
  const summary = text.trim()
    ? `${report.entries.length} 条释义` + (report.skipped ? `，${report.skipped} 行无法识别` : "")
    : "还没有自定义释义。";

  const mounted = useMountedRef();
  const clientRef = useRef(client);
  clientRef.current = client;
  // The text as last edited, and whether it differs from what was last written; the save loop reads these so edits made while a save is in flight are not lost.
  const textRef = useRef("");
  const dirtyRef = useRef(false);
  const savingRef = useRef(false);
  const autosaveTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const savedStatusTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    if (!client) return;
    let active = true;
    void client
      .load()
      .then((value) => {
        // An edit made before the file arrived wins over it rather than being overwritten.
        if (!active || dirtyRef.current) return;
        textRef.current = value;
        setTextState(value);
      })
      .catch(() => {
        // An unreadable overlay stays empty; saving it creates a fresh valid file.
      });
    return () => {
      active = false;
    };
  }, [client]);

  function clearAutosave() {
    if (autosaveTimer.current === undefined) return;
    clearTimeout(autosaveTimer.current);
    autosaveTimer.current = undefined;
  }

  /** Writes the latest text if it has not been written yet, then keeps writing while edits made during the save are still unsaved. Only one save runs at a time. */
  async function flush() {
    clearAutosave();
    const current = clientRef.current;
    if (!current || !mounted.current || savingRef.current || !dirtyRef.current) return;
    if (!customTranslationsWithinBounds(textRef.current)) {
      // Nothing is written until the text fits again; the next edit schedules another attempt.
      setNotice(oversizedNotice);
      setSaveState("idle");
      return;
    }
    savingRef.current = true;
    clearTimeout(savedStatusTimer.current);
    setSaveState("saving");
    setSaveError("");
    let failed = false;
    try {
      // Keep draining edits even if the component unmounts while the current write is in flight.
      // The client call itself is independent of React state, and dropping this loop on unmount
      // would lose text entered after the first request started.
      while (dirtyRef.current) {
        const sent = textRef.current;
        if (!customTranslationsWithinBounds(sent)) break;
        dirtyRef.current = false;
        try {
          await current.save(sent);
        } catch (reason) {
          // The edit is still unsaved; 重试 or the next edit writes it again.
          dirtyRef.current = true;
          throw reason;
        }
      }
    } catch (reason) {
      failed = true;
      if (mounted.current) {
        setSaveState("failed");
        setSaveError(errorMessage(reason));
      }
    } finally {
      savingRef.current = false;
    }
    if (!mounted.current || failed) return;
    if (dirtyRef.current) {
      // Only an oversized edit made during the save stops the loop early; say so instead of claiming it was saved.
      setNotice(oversizedNotice);
      setSaveState("idle");
      return;
    }
    setSaveState("saved");
    savedStatusTimer.current = setTimeout(
      () => setSaveState((state) => (state === "saved" ? "idle" : state)),
      SETTINGS_SAVED_STATUS_MS,
    );
  }
  const flushRef = useRef(flush);
  flushRef.current = flush;
  useFlushOnWindowLeave(() => void flushRef.current());

  function setText(value: string) {
    textRef.current = value;
    dirtyRef.current = true;
    setTextState(value);
    setNotice("");
    setSaveState((state) => (state === "saved" ? "idle" : state));
    // A save in flight picks the new text up when it finishes; otherwise the countdown restarts.
    if (savingRef.current) return;
    clearAutosave();
    autosaveTimer.current = setTimeout(() => {
      autosaveTimer.current = undefined;
      void flushRef.current();
    }, SETTINGS_AUTOSAVE_DELAY_MS);
  }

  // Unmounting with an edit still counting down writes it without waiting for an answer: nothing is left on screen to show the result.
  useEffect(
    () => () => {
      clearTimeout(savedStatusTimer.current);
      if (autosaveTimer.current === undefined) return;
      clearAutosave();
      const current = clientRef.current;
      const pending = textRef.current;
      if (!current || savingRef.current || !dirtyRef.current) return;
      if (!customTranslationsWithinBounds(pending)) return;
      dirtyRef.current = false;
      void Promise.resolve()
        .then(() => current.save(pending))
        .catch(() => undefined);
    },
    [],
  );

  return {
    text,
    setText,
    notice,
    summary,
    saveState,
    saveError,
    placeholder: customTranslationsExample,
    flush,
  } as const;
}
