import { useEffect, useMemo, useState } from "react";
import { errorMessage } from "../core/error-message";
import {
  customTranslationsExample,
  customTranslationsWithinBounds,
  parseCustomTranslations,
} from "../dictionary/custom-translations";

export interface CustomTranslationsClient {
  load(): Promise<string>;
  save(text: string): Promise<void>;
}

export interface UseCustomTranslationsOptions {
  client?: CustomTranslationsClient;
}

/** Owns loading, validation, saving, and summary text for user translation overlays. */
export function useCustomTranslations({ client }: UseCustomTranslationsOptions) {
  const [text, setText] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const report = useMemo(() => parseCustomTranslations(text), [text]);
  const summary = text.trim()
    ? `${report.entries.length} 条释义` + (report.skipped ? `，${report.skipped} 行无法识别` : "")
    : "还没有自定义释义。";

  useEffect(() => {
    if (!client) return;
    let active = true;
    void client
      .load()
      .then((value) => {
        if (active) setText(value);
      })
      .catch(() => {
        // An unreadable overlay stays empty; saving it creates a fresh valid file.
      });
    return () => {
      active = false;
    };
  }, [client]);

  async function save() {
    if (!client || busy) return;
    if (!customTranslationsWithinBounds(text)) {
      setNotice("自定义释义过大，请精简后再保存。");
      return;
    }
    setBusy(true);
    try {
      await client.save(text);
      setNotice(`已保存 ${report.entries.length} 条释义，重新启动输入法后生效。`);
    } catch (reason) {
      setNotice(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  return {
    text,
    setText: (value: string) => {
      setText(value);
      setNotice("");
    },
    notice,
    summary,
    busy,
    placeholder: customTranslationsExample,
    save,
  } as const;
}
