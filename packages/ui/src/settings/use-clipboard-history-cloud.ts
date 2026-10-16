import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import { cloudClipboardE2e } from "../keyboard/cloud-response";
import {
  CLOUD_CLIPBOARD_MAX_UTF16,
  cloudClipboardAvailabilityNote,
  cloudClipboardErrorMessage,
  cloudClipboardFailure,
  type CloudClipboardAvailability,
  type CloudClipboardRequest,
} from "./cloud-clipboard-send";

export interface UseClipboardHistoryCloudOptions {
  cloudRequest?: CloudClipboardRequest;
  cloudRoute?: string;
  historyShown: boolean;
}

export interface ClipboardHistoryCloudState {
  cloud: CloudClipboardAvailability;
  cloudNotice: string;
  sending: boolean;
  cloudNote: string | undefined;
  sendToCloud: (text: string) => Promise<void> | undefined;
}

/** 查询一次当前路由的可用状态，并保护手动发送的互斥与结果归属。 */
export function useClipboardHistoryCloud({
  cloudRequest,
  cloudRoute,
  historyShown,
}: UseClipboardHistoryCloudOptions): ClipboardHistoryCloudState {
  const [cloud, setCloud] = useState<CloudClipboardAvailability>("checking");
  const [cloudNotice, setCloudNotice] = useState("");
  const {
    run,
    busy: sending,
    running,
    generation: cloudRevision,
  } = useAsyncActionRunner(setCloudNotice, undefined, cloudRequest, historyShown, cloudRoute);

  useEffect(() => {
    const revision = cloudRevision.current;
    setCloud("checking");
    setCloudNotice("");
    if (!cloudRequest || !historyShown) return;
    void cloudRequest({ operation: "list", search: "" }).then(
      (result) => {
        if (revision !== cloudRevision.current) return;
        const e2e = cloudClipboardE2e(result);
        setCloud(
          result.enabled === false || e2e?.status === "disabled"
            ? "disabled"
            : e2e?.status === "pending"
              ? "pending"
              : "ready",
        );
      },
      (error: unknown) => {
        if (revision === cloudRevision.current) setCloud(cloudClipboardFailure(error));
      },
    );
  }, [cloudRequest, cloudRevision, historyShown, cloudRoute]);

  const sendToCloud = (text: string) => {
    if (!cloudRequest || !historyShown || cloud !== "ready" || running.current) return;
    if (text.length > CLOUD_CLIPBOARD_MAX_UTF16) {
      setCloudNotice(`这条记录超过 ${CLOUD_CLIPBOARD_MAX_UTF16} 个 UTF-16 单元，无法发到云剪贴板`);
      return;
    }
    return run(
      async (isCurrent) => {
        setCloudNotice("正在发到云剪贴板…");
        await cloudRequest({ operation: "add", text });
        if (isCurrent()) setCloudNotice("已发到云剪贴板");
      },
      {
        formatError: (error) =>
          cloudClipboardFailure(error) === "unavailable"
            ? cloudClipboardErrorMessage(error, "发到云剪贴板失败，请稍后重试")
            : "",
        onError: (error) => {
          const failure = cloudClipboardFailure(error);
          // 阻塞性错误需要用户先处理，普通服务错误仍允许重试。
          if (failure !== "unavailable") setCloud(failure);
        },
      },
    );
  };
  return {
    cloud,
    cloudNotice,
    sending,
    cloudNote: cloudRequest ? cloudNotice || cloudClipboardAvailabilityNote(cloud) : undefined,
    sendToCloud,
  };
}
