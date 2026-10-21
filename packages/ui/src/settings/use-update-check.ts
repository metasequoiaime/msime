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
  /** 宿主后台检查已经找到的新版本（`SettingsClient.pendingUpdateCheck`），页面打开时先显示它。 */
  pendingUpdateCheck?: () => Promise<UpdateCheckResult | null>;
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
  pendingUpdateCheck,
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

  // 后台检查已经找到新版本时直接显示，与手动检查发现新版本时一样；读不到或没有时保持原样。
  useEffect(() => {
    if (!pendingUpdateCheck) return;
    let active = true;
    void pendingUpdateCheck()
      .then((result) => {
        if (!active || result?.status !== "available") return;
        const update = fromHostUpdate(result.update);
        if (!update) return;
        setAvailable((current) => current ?? update);
        setStatus((current) => current || `发现新版本 v${update.version.display}`);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
    // 上面的重置在这些值变化时清空结果，这里随之重新读取一次。
  }, [
    arch,
    checkUpdate,
    checkForApplicationUpdate,
    currentAppVersion,
    edition,
    pendingUpdateCheck,
    platform,
  ]);

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
