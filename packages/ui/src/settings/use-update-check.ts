import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import {
  fromHostUpdate,
  type UpdateCheckRequest,
  type UpdateCheckResult,
  type ValidatedUpdate,
} from "./update-manifest";

export interface UseUpdateCheckOptions {
  /** 完整应用的原生更新流程，更新窗口由宿主呈现。 */
  checkForApplicationUpdate?: () => Promise<void>;
  /** 宿主的检查更新（`SettingsClient.checkUpdate`），经由 `msime_client_services::update_check` 询问 GitHub。未提供时页面不提供检查。 */
  checkUpdate?: (request: UpdateCheckRequest) => Promise<UpdateCheckResult>;
  /** 宿主所在平台（`HostCapabilities.platform`），同时也是它的发布 tag 前缀。 */
  platform: string | null;
  /** 运行中的版本 id（`HostCapabilities.edition.id`），缺省是 full：只选本版本的安装包。 */
  edition?: string;
  /** The host's architecture (`HostCapabilities.arch`): a Linux release carries one package per architecture. */
  arch?: string;
  currentAppVersion: string;
}

/** 询问宿主本平台、本版本有没有更新的已发布版本，并保存关于页展示的状态。 */
export function useUpdateCheck({
  checkUpdate,
  checkForApplicationUpdate,
  platform,
  edition,
  arch,
  currentAppVersion,
}: UseUpdateCheckOptions) {
  const [status, setStatus] = useState("");
  const [available, setAvailable] = useState<ValidatedUpdate | null>(null);
  const supported = !!checkForApplicationUpdate || (!!checkUpdate && !!platform);
  const { busy, run } = useAsyncActionRunner(
    setStatus,
    undefined,
    checkUpdate,
    checkForApplicationUpdate,
    currentAppVersion,
    edition,
    arch,
    platform,
  );

  useEffect(() => {
    setStatus("");
    setAvailable(null);
  }, [arch, checkUpdate, checkForApplicationUpdate, currentAppVersion, edition, platform]);

  async function checkForUpdate() {
    if (busy) return;
    if (checkForApplicationUpdate) {
      setAvailable(null);
      setStatus("");
      await run(
        async () => {
          await checkForApplicationUpdate();
        },
        {
          formatError: () => "无法打开自动更新，请稍后重试",
        },
      );
      return;
    }
    if (!checkUpdate || !platform) return;
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
