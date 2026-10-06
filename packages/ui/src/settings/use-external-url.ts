export interface UseExternalUrlOptions {
  openExternalUrl?: (url: string) => Promise<void>;
  setError: (error: string) => void;
}

function isSafeExternalUrl(url: string): boolean {
  if (
    url.length > 4096 ||
    !url.startsWith("https://") ||
    url.slice("https://".length).startsWith("/") ||
    url.split("").some((character) => {
      const code = character.charCodeAt(0);
      return code <= 0x20 || "\"'`|<>\\".includes(character);
    })
  )
    return false;

  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return false;
  }

  return (
    parsed.protocol === "https:" &&
    parsed.hostname !== "" &&
    parsed.username === "" &&
    parsed.password === ""
  );
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
        if (!isSafeExternalUrl(url)) throw new Error("invalid URL");
        const opened = window.open(url, "_blank", "noopener,noreferrer");
        if (!opened) throw new Error("popup blocked");
      }
    } catch {
      setError("无法打开外部链接，请稍后重试。");
    }
  }

  return openExternalUrl;
}
