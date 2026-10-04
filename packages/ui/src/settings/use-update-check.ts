import { useEffect, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
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
import { useAsyncGeneration } from "./use-async-generation";

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
  const actionRunning = useRef(false);
  const requestGeneration = useAsyncGeneration(
    clientHostedPlatform,
    currentAppVersion,
    releasePageUrl,
    releasePlatform,
  );

  useEffect(() => {
    actionRunning.current = false;
    setStatus("");
    setBusy(false);
    setAvailable(null);
  }, [clientHostedPlatform, currentAppVersion, releasePageUrl, releasePlatform]);

  async function checkForUpdate() {
    if (busy || actionRunning.current) return;
    const generation = requestGeneration.current;
    actionRunning.current = true;
    setAvailable(null);
    try {
      await runAsyncAction(
        {
          busy,
          isCurrent: () => generation === requestGeneration.current,
          setBusy,
          setError: setStatus,
        },
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
              update = selectPlatformRelease(manifest, releasePlatform, releasePageUrl);
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
    } finally {
      if (generation === requestGeneration.current) actionRunning.current = false;
    }
  }

  return { available, busy, checkForUpdate, status } as const;
}
