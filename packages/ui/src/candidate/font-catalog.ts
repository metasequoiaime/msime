import { useEffect, useMemo, useState } from "react";
import { validFontFamily } from "./candidate-font-family";
import { useAsyncGeneration } from "../settings/use-async-generation";
export type FontCatalogReader = () => Promise<string[]>;
export function normalizeFontCatalog(value: unknown): string[] {
  if (!Array.isArray(value) || value.length > 16384 || !value.every(validFontFamily))
    throw new Error("invalid font catalog");
  return [...new Set(value)].sort((a, b) => a.localeCompare(b));
}
export function useFontCatalog(read?: FontCatalogReader) {
  const [requested, setRequested] = useState(false),
    [revision, setRevision] = useState(0);
  const key = useMemo(() => ({}), [read, revision]);
  const [result, setResult] = useState<{ key: object; fonts: string[]; failed?: boolean }>();
  const generation = useAsyncGeneration(read, revision, requested);
  useEffect(() => {
    const requestGeneration = generation.current;
    if (requested && read)
      void (async () => {
        try {
          const fonts = normalizeFontCatalog(await read());
          if (generation.current === requestGeneration) setResult({ key, fonts });
        } catch {
          if (generation.current === requestGeneration) setResult({ key, fonts: [], failed: true });
        }
      })();
  }, [read, key, requested, generation]);
  const current = result?.key === key ? result : undefined;
  const status = !read
    ? "unsupported"
    : !requested
      ? "idle"
      : !current
        ? "loading"
        : current.failed
          ? "failed"
          : "ready";
  return {
    fonts: current?.fonts ?? [],
    status,
    request: () => setRequested(true),
    refresh: () => {
      setRequested(true);
      setRevision((value) => value + 1);
    },
  };
}
