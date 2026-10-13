import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HandwritingPanel, type PanelClient } from "@msime/ui";

/** macOS 设置中打开的手写板只识别并复制；原生会话确认可上屏后才提供输入。 */
export function DesktopHandwriting({
  client,
  theme,
  platform,
}: {
  client: PanelClient;
  theme: "dark" | "light";
  platform?: string;
}) {
  const [inputOwner, setInputOwner] = useState<PanelClient | null>(null);
  useEffect(() => {
    let current = true;
    setInputOwner(null);
    if (platform === "macos")
      void invoke<boolean>("handwriting_can_input")
        .then((allowed) => {
          if (current && allowed) setInputOwner(client);
        })
        .catch(() => {});
    return () => {
      current = false;
    };
  }, [client, platform]);
  const effectiveClient = useMemo(
    () =>
      platform === undefined || (platform === "macos" && inputOwner !== client)
        ? { ...client, submitHandwritingCandidate: undefined }
        : client,
    [client, platform, inputOwner],
  );
  return <HandwritingPanel client={effectiveClient} theme={theme} platform={platform} />;
}
