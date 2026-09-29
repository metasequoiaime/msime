export interface UseExternalUrlOptions {
  openExternalUrl?: (url: string) => Promise<void>;
  setError: (error: string) => void;
}

/** Opens links through the native host when available and falls back to a browser window. */
export function useExternalUrl({
  openExternalUrl: hostOpenExternalUrl,
  setError,
}: UseExternalUrlOptions) {
  async function openExternalUrl(url: string) {
    try {
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
