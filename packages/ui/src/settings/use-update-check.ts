import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import {
  compareVersions,
  parseVersion,
  selectPlatformRelease,
  validateManifest,
  type GitHubRelease,
  type UpdateManifest,
  type ValidatedUpdate,
} from "./update-manifest";
import { UPDATE_CHECK_TIMEOUT_MS, clientReleasesUrl, updateManifestUrl } from "./app-resources";

export interface UseUpdateCheckOptions {
  clientHostedPlatform: boolean;
  releasePlatform: string | null;
  /** 运行中的版本 id（`HostCapabilities.edition.id`），缺省是 full：只选本版本的安装包。 */
  edition?: string;
  /** The host's architecture (`HostCapabilities.arch`): a Linux release carries one package per architecture. */
  arch?: string;
  releasePageUrl: string;
  currentAppVersion: string;
}

/** Owns release-feed fetching, validation, timeout handling, and update status. */
export function useUpdateCheck({
  clientHostedPlatform,
  releasePlatform,
  edition,
  arch,
  releasePageUrl,
  currentAppVersion,
}: UseUpdateCheckOptions) {
  const [status, setStatus] = useState("");
  const [available, setAvailable] = useState<ValidatedUpdate | null>(null);
  const { busy, run } = useAsyncActionRunner(
    setStatus,
    undefined,
    clientHostedPlatform,
    currentAppVersion,
    edition,
    arch,
    releasePageUrl,
    releasePlatform,
  );

  useEffect(() => {
    setStatus("");
    setAvailable(null);
  }, [arch, clientHostedPlatform, currentAppVersion, edition, releasePageUrl, releasePlatform]);

  async function checkForUpdate() {
    if (busy) return;
    setAvailable(null);
    await run(
      async (isCurrent) => {
        const controller = new AbortController();
        const timeout = window.setTimeout(() => controller.abort(), UPDATE_CHECK_TIMEOUT_MS);
        try {
          const endpoint =
            clientHostedPlatform && releasePlatform
              ? `${clientReleasesUrl}?per_page=100&t=${Date.now()}`
              : `${updateManifestUrl}?t=${Date.now()}`;
          const response = await fetch(endpoint, {
            cache: "no-store",
            signal: controller.signal,
          });
          if (!response.ok) throw new Error(`update manifest returned ${response.status}`);
          const manifest = (await response.json()) as UpdateManifest | GitHubRelease[];
          let update: ValidatedUpdate | null;
          if (clientHostedPlatform && releasePlatform) {
            if (!Array.isArray(manifest)) throw new Error("invalid release list");
            update = selectPlatformRelease(
              manifest,
              releasePlatform,
              releasePageUrl,
              edition,
              arch,
            );
            if (!update) {
              if (isCurrent()) setStatus("暂无可用发行版");
              return;
            }
          } else {
            update = validateManifest(manifest as UpdateManifest, releasePageUrl);
          }
          const current = parseVersion(currentAppVersion);
          if (!update || !current) throw new Error("invalid update manifest");
          if (!isCurrent()) return;
          if (compareVersions(update.version, current) > 0) {
            setAvailable(update);
            setStatus(`发现新版本 v${update.version.display}`);
          } else {
            setStatus("已是最新版本");
          }
        } finally {
          window.clearTimeout(timeout);
        }
      },
      { formatError: () => "检查失败，请稍后重试" },
    );
  }

  return { available, busy, checkForUpdate, status } as const;
}
