import { useEffect, useState } from "react";
import { useLatestRef } from "../core/use-latest-ref";
import { useAsyncActionRunner } from "../core/use-async-action";
import type {
  CandidateSkinCommunityClient,
  CandidateSkinSyncReport,
} from "./community-candidate-skin-model";

export type CandidateSkinSyncState = {
  busy: boolean;
  report: CandidateSkinSyncReport | null;
  error: unknown;
  /** 手动开始同步；运行中忽略重复请求。 */
  run: () => void;
};

/**
 * 只有调用 `run` 才同步本机目录和账号皮肤库；页面挂载或客户端变化不触发同步。下载或删除本机包后用 `onChanged` 通知目录重扫。
 */
export function useCandidateSkinSync(
  client: CandidateSkinCommunityClient | undefined,
  onChanged: () => void,
): CandidateSkinSyncState {
  const [report, setReport] = useState<CandidateSkinSyncReport | null>(null);
  const [error, setError] = useState<unknown>(null);
  const changed = useLatestRef(onChanged);
  const {
    busy,
    mounted,
    running,
    generation: clientGeneration,
    run: runAsyncAction,
  } = useAsyncActionRunner(() => {}, undefined, client);

  const run = () => {
    if (!client || running.current) return;
    setReport(null);
    setError(null);
    const current = clientGeneration.current;
    void runAsyncAction(
      async (isCurrent) => {
        try {
          const result = await client.sync();
          if (!isCurrent()) return;
          setReport(result);
          setError(null);
          if (result.downloaded.length || result.deleted_local.length) changed.current();
        } catch (failure) {
          if (mounted.current && current === clientGeneration.current) setError(failure);
        }
      },
      { formatError: () => "" },
    );
  };

  useEffect(() => {
    setReport(null);
    setError(null);
  }, [client, clientGeneration]);

  return { busy, report, error, run };
}
