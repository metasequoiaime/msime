import { useCallback, useEffect, useState } from "react";
import { useToast } from "../core/toast";
import { useAsyncActionRunner } from "../core/use-async-action";
import { resourceMessage } from "./community-helpers";
import { addCommunityReply } from "./community-resource-actions";
import type { CommunityResource, CommunityResourceClient } from "./community-resource-model";

export interface UseCommunityReplyTemplatesOptions {
  client: CommunityResourceClient;
  /** 除 toast 外也在预览区显示添加结果，供没有 toast 的宿主使用。 */
  inlineNotice?: boolean;
}

/** AI 辅助页还包含其他设置，预览只取前五条回复模板。 */
const REPLY_PREVIEW_LIMIT = 5;

/** 社区回复模板的预览读取、添加与提示状态，独立于设置页导航。 */
export function useCommunityReplyTemplates({
  client,
  inlineNotice = true,
}: UseCommunityReplyTemplatesOptions) {
  const toast = useToast();
  const [items, setItems] = useState<CommunityResource[] | null>(null);
  const [failure, setFailure] = useState("");
  const [notice, setNotice] = useState("");
  const [adding, setAdding] = useState<string | null>(null);
  // 只记录本次挂载且属于当前客户端的添加；宿主不提供已添加模板清单。
  const [added, setAdded] = useState<ReadonlySet<string>>(() => new Set());
  const notify = useCallback(
    (message: string) => {
      if (!message) return;
      toast(message);
      if (inlineNotice) setNotice(message);
    },
    [toast, inlineNotice],
  );
  const { busy, run, generation } = useAsyncActionRunner(notify, undefined, client);

  useEffect(() => {
    const current = generation.current;
    setItems(null);
    setFailure("");
    setNotice("");
    setAdding(null);
    setAdded(new Set());
    client
      .list("reply", "", "", 0)
      .then(
        (page) =>
          generation.current === current && setItems(page.items.slice(0, REPLY_PREVIEW_LIMIT)),
      )
      .catch(
        () => generation.current === current && setFailure("社区回复模板暂时读不到，请稍后再试。"),
      );
  }, [client]);

  const add = (item: CommunityResource) =>
    run(
      async (isCurrent) => {
        setAdding(item.id);
        setNotice("");
        const message = await addCommunityReply(client, item);
        if (!isCurrent()) return;
        setAdded((current) => new Set(current).add(item.id));
        notify(message);
      },
      { formatError: resourceMessage },
    );

  return { items, failure, notice, adding: busy ? adding : null, added, add };
}

export type CommunityReplyTemplatesState = ReturnType<typeof useCommunityReplyTemplates>;
