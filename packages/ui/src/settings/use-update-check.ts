import { useState } from "react";
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
  releasePageUrl: string;
  currentAppVersion: string;
}

/** Owns release-feed fetching, validation, timeout handling, and update status. */
export function useUpdateCheck({
  clientHostedPlatform,
  releasePlatform,
  releasePageUrl,
  currentAppVersion,
}: UseUpdateCheckOptions) {
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const [available, setAvailable] = useState<ValidatedUpdate | null>(null);

  async function checkForUpdate() {
    setBusy(true);
    setStatus("");
    setAvailable(null);
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), UPDATE_CHECK_TIMEOUT_MS);
    try {
      const endpoint =
        clientHostedPlatform && releasePlatform
          ? `${clientReleasesUrl}?per_page=100&t=${Date.now()}`
          : `${updateManifestUrl}?t=${Date.now()}`;
      const response = await fetch(endpoint, { cache: "no-store", signal: controller.signal });
      if (!response.ok) throw new Error(`update manifest returned ${response.status}`);
      const manifest = (await response.json()) as UpdateManifest | GitHubRelease[];
      let update: ValidatedUpdate | null;
      if (clientHostedPlatform && releasePlatform) {
        if (!Array.isArray(manifest)) throw new Error("invalid release list");
        update = selectPlatformRelease(manifest, releasePlatform, releasePageUrl);
        if (!update) {
          setStatus("暂无可用发行版");
          return;
        }
      } else {
        update = validateManifest(manifest as UpdateManifest, releasePageUrl);
      }
      const current = parseVersion(currentAppVersion);
      if (!update || !current) throw new Error("invalid update manifest");
      if (compareVersions(update.version, current) > 0) {
        setAvailable(update);
        setStatus(`发现新版本 v${update.version.display}`);
      } else {
        setStatus("已是最新版本");
      }
    } catch {
      setStatus("检查失败，请稍后重试");
    } finally {
      window.clearTimeout(timeout);
      setBusy(false);
    }
  }

  return { available, busy, checkForUpdate, status } as const;
}
