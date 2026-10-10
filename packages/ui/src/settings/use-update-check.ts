import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import {
  fromHostUpdate,
  type UpdateCheckRequest,
  type UpdateCheckResult,
  type ValidatedUpdate,
} from "./update-manifest";

export interface UseUpdateCheckOptions {
  /** The host's update check (`SettingsClient.checkUpdate`), which asks GitHub through `msime_client_core::update_check`. Absent, the page offers no check. */
  checkUpdate?: (request: UpdateCheckRequest) => Promise<UpdateCheckResult>;
  /** The host's platform (`HostCapabilities.platform`), which is also its release tag prefix. */
  platform: string | null;
  /** 运行中的版本 id（`HostCapabilities.edition.id`），缺省是 full：只选本版本的安装包。 */
  edition?: string;
  /** The host's architecture (`HostCapabilities.arch`): a Linux release carries one package per architecture. */
  arch?: string;
  currentAppVersion: string;
}

/** Asks the host whether a newer release of this platform and edition is published, and keeps the status the about page shows. */
export function useUpdateCheck({
  checkUpdate,
  platform,
  edition,
  arch,
  currentAppVersion,
}: UseUpdateCheckOptions) {
  const [status, setStatus] = useState("");
  const [available, setAvailable] = useState<ValidatedUpdate | null>(null);
  const supported = !!checkUpdate && !!platform;
  const { busy, run } = useAsyncActionRunner(
    setStatus,
    undefined,
    checkUpdate,
    currentAppVersion,
    edition,
    arch,
    platform,
  );

  useEffect(() => {
    setStatus("");
    setAvailable(null);
  }, [arch, checkUpdate, currentAppVersion, edition, platform]);

  async function checkForUpdate() {
    if (busy || !checkUpdate || !platform) return;
    setAvailable(null);
    await run(
      async (isCurrent) => {
        const result = await checkUpdate({
          platform,
          currentVersion: currentAppVersion,
          ...(edition === undefined ? {} : { edition }),
          ...(arch === undefined ? {} : { arch }),
        });
        if (!isCurrent()) return;
        if (result.status === "none") {
          setStatus("暂无可用发行版");
          return;
        }
        const update = fromHostUpdate(result.update);
        if (!update || (result.status !== "available" && result.status !== "current"))
          throw new Error("invalid update check result");
        if (result.status === "available") {
          setAvailable(update);
          setStatus(`发现新版本 v${update.version.display}`);
        } else {
          setStatus("已是最新版本");
        }
      },
      { formatError: () => "检查失败，请稍后重试" },
    );
  }

  return { available, busy, checkForUpdate, status, supported } as const;
}
