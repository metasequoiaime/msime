import { useEffect, useRef, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import {
  describeUpdateVersion,
  fromHostUpdate,
  type UpdateChannel,
  type UpdateCheckRequest,
  type UpdateCheckResult,
  type ValidatedUpdate,
} from "./update-manifest";

export interface UseUpdateCheckOptions {
  /** 完整应用的原生更新流程，更新窗口由宿主呈现。给出了 `channel` 时一并传给宿主，按这个通道的更新 feed 检查。 */
  checkForApplicationUpdate?: (channel?: UpdateChannel) => Promise<void>;
  /** 宿主的检查更新（`SettingsClient.checkUpdate`），经由 `msime_client_services::update_check` 询问 GitHub。未提供时页面不提供检查。 */
  checkUpdate?: (request: UpdateCheckRequest) => Promise<UpdateCheckResult>;
  /** 宿主后台检查已经找到的新版本（`SettingsClient.pendingUpdateCheck`），页面打开时先显示它；给出了 `channel` 时一并传给宿主，只要按这个通道找到的版本。 */
  pendingUpdateCheck?: (channel?: UpdateChannel) => Promise<UpdateCheckResult | null>;
  /** 宿主所在平台（`HostCapabilities.platform`），同时也是它的发布 tag 前缀。 */
  platform: string | null;
  /** 运行中的版本 id（`HostCapabilities.edition.id`），缺省是 full：只选本版本的安装包。 */
  edition?: string;
  /** The host's architecture (`HostCapabilities.arch`): a Linux release carries one package per architecture. */
  arch?: string;
  /** 偏好里的更新通道，只有提供通道选择的宿主（macOS、Linux）才给；不给时请求里不带通道，宿主按稳定版检查。换了通道时清掉按旧通道找到的结果。 */
  channel?: UpdateChannel;
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
  channel,
  currentAppVersion,
}: UseUpdateCheckOptions) {
  const [status, setStatus] = useState("");
  const [available, setAvailable] = useState<ValidatedUpdate | null>(null);
  // 从后台记录读到的那个结果；`available` 正是它（同一个对象）时，页面显示的是记录而不是这次打开后检查的结果。
  const [recorded, setRecorded] = useState<ValidatedUpdate | null>(null);
  const manualCheckStarted = useRef(false);
  const supported = !!checkForApplicationUpdate || (!!checkUpdate && !!platform);
  const { busy, run } = useAsyncActionRunner(
    setStatus,
    undefined,
    checkUpdate,
    checkForApplicationUpdate,
    currentAppVersion,
    edition,
    arch,
    channel,
    platform,
  );

  useEffect(() => {
    manualCheckStarted.current = false;
    setStatus("");
    setAvailable(null);
    setRecorded(null);
  }, [arch, channel, checkUpdate, checkForApplicationUpdate, currentAppVersion, edition, platform]);

  // 后台检查已经找到新版本时直接显示，与手动检查发现新版本时一样；读不到或没有时保持原样。
  useEffect(() => {
    if (!pendingUpdateCheck) return;
    let active = true;
    void (channel === undefined ? pendingUpdateCheck() : pendingUpdateCheck(channel))
      .then((result) => {
        if (!active || manualCheckStarted.current || result?.status !== "available") return;
        const update = fromHostUpdate(result.update);
        if (!update) return;
        setRecorded(update);
        setAvailable((current) => current ?? update);
        setStatus((current) => current || `发现新版本 ${describeUpdateVersion(update)}`);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
    // 上面的重置在这些值变化时清空结果，这里随之重新读取一次。
  }, [
    arch,
    channel,
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
      await run(
        async () => {
          manualCheckStarted.current = true;
          setAvailable(null);
          await (channel === undefined
            ? checkForApplicationUpdate()
            : checkForApplicationUpdate(channel));
        },
        {
          formatError: () => "无法打开自动更新，请稍后重试",
        },
      );
      return;
    }
    if (!checkUpdate || !platform) return;
    await run(
      async (isCurrent) => {
        // 手动检查接管展示，迟到的后台缓存不能恢复已清掉的旧结果。
        manualCheckStarted.current = true;
        setAvailable(null);
        const result = await checkUpdate({
          platform,
          currentVersion: currentAppVersion,
          ...(edition === undefined ? {} : { edition }),
          ...(arch === undefined ? {} : { arch }),
          ...(channel === undefined ? {} : { channel }),
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
          setStatus(`发现新版本 ${describeUpdateVersion(update)}`);
        } else {
          setStatus("已是最新版本");
        }
      },
      { formatError: () => "检查失败，请稍后重试" },
    );
  }

  // 显示的是后台记录里的测试版或每日构建时为 true：每日构建每天重新上传、删掉不再引用的旧安装包，测试版重跑会原地换掉同名安装包，而后台记录最多一天才刷新一次，记录里的安装包名和 SHA256 可能已经对不上发布页。这时页面不给镜像链接和校验值，请用户点「检查更新」拿到当前的。稳定版的发布不会被替换，照常显示。
  const installerDetailsStale =
    available !== null && available === recorded && available.channel !== "stable";

  return { available, busy, checkForUpdate, installerDetailsStale, status, supported } as const;
}
