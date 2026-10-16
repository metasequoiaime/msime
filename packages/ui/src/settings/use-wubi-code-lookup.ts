import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import type { WubiCodeLookup, WubiCodeLookupClient } from "../dictionary/dictionary-contracts";

export interface UseWubiCodeLookupOptions {
  lookup: WubiCodeLookupClient;
}

export interface WubiCodeLookupState {
  text: string;
  result: WubiCodeLookup | null;
  error: string;
  busy: boolean;
  canSearch: boolean;
  edit: (value: string) => void;
  search: () => Promise<void> | undefined;
}

/** 查码状态独立于输入框与结果表；编辑或更换客户端后隔离旧请求。 */
export function useWubiCodeLookup({ lookup }: UseWubiCodeLookupOptions): WubiCodeLookupState {
  const [text, setText] = useState("");
  const [result, setResult] = useState<WubiCodeLookup | null>(null);
  const [error, setError] = useState("");
  const { busy, invalidate, run } = useAsyncActionRunner(setError, undefined, lookup);

  useEffect(() => {
    setResult(null);
    setError("");
  }, [lookup]);

  const edit = (value: string) => {
    invalidate();
    setText(value);
    setResult(null);
    setError("");
  };
  const canSearch = !busy && Boolean(text.trim());
  const search = () => {
    if (!canSearch) return;
    return run(
      async (isCurrent) => {
        const answer = await lookup(text);
        if (isCurrent()) setResult(answer);
      },
      {
        formatError: () => "查码失败，请稍后重试。",
        onError: () => setResult(null),
      },
    );
  };

  return { text, result, error, busy, canSearch, edit, search };
}
