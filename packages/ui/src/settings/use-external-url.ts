export interface UseExternalUrlOptions {
  openExternalUrl?: (url: string) => Promise<void>;
  setError: (error: string) => void;
}

/** External links leave the app; keep both native and browser fallbacks HTTPS-only. */
export function isSafeExternalUrl(url: string): boolean {
  if (new TextEncoder().encode(url).length > 4096 || /[\u0000-\u0020\u007f-\u009f]/.test(url)) {
    return false;
  }
  try {
    const parsed = new URL(url);
    return parsed.protocol === "https:" && Boolean(parsed.hostname) &&
      !parsed.username && !parsed.password && !/["'`&|<>\\]/.test(url);
  } catch {
    return false;
  }
}

/** Opens links through the native host when available and falls back to a browser window. */
export function useExternalUrl({
  openExternalUrl: hostOpenExternalUrl,
  setError,
}: UseExternalUrlOptions) {
  async function openExternalUrl(url: string) {
    try {
      if (!isSafeExternalUrl(url)) throw new Error("invalid URL");
      if (hostOpenExternalUrl) {
        await hostOpenExternalUrl(url);
      } else {
        const opened = window.open(url, "_blank", "noopener,noreferrer");
        if (!opened) throw new Error("popup blocked");
      }
    } catch {
      setError("无法打开外部链接，请稍后重试。");
    }
  }

  return openExternalUrl;
}
