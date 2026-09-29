export interface UseOpenPanelOptions {
  setError: (error: string) => void;
}

/** Wraps optional native panel commands with the shared settings error messages. */
export function useOpenPanel({ setError }: UseOpenPanelOptions) {
  async function openPanel(action: (() => Promise<void>) | undefined) {
    if (!action) {
      setError("当前宿主未接入该原生面板。");
      return;
    }
    try {
      await action();
    } catch {
      setError("无法打开原生面板，请稍后重试。");
    }
  }

  return openPanel;
}
