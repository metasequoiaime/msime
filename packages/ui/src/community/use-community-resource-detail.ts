import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import { resourceMessage } from "./community-helpers";
import { communityReportedNotice, type CommunityReportReason } from "./community-report-model";
import { installCommunityDictionaryLocally } from "./community-resource-actions";
import type {
  CommunityResourceClient,
  CommunityResource,
  CommunityLocalDictionaryClient,
} from "./community-resource-model";

export interface CommunityResourceDetailOptions {
  client: Pick<
    CommunityResourceClient,
    "detail" | "save" | "apply" | "rate" | "unpublish" | "storeReply" | "removeReply" | "report"
  >;
  initial: CommunityResource;
  onClose: () => void;
  localDictionary?: CommunityLocalDictionaryClient;
}

/** 资源详情的加载与动作；每一步完成后保留客户端、资源和挂载代次检查。 */
export function useCommunityResourceDetail({
  client,
  initial,
  onClose,
  localDictionary,
}: CommunityResourceDetailOptions) {
  const [item, setItem] = useState(initial);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const {
    busy,
    mounted,
    generation: clientGeneration,
    run: runAsyncAction,
  } = useAsyncActionRunner(setError, setNotice, client, initial.id);
  const renderGeneration = clientGeneration.current;
  const run = (action: (generation: number) => Promise<void>) => {
    const generation = clientGeneration.current;
    return runAsyncAction(() => action(generation), {
      formatError: resourceMessage,
    });
  };
  useEffect(() => {
    const generation = clientGeneration.current;
    void client
      .detail(initial.id)
      .then((value) => {
        if (mounted.current && generation === clientGeneration.current) setItem(value);
      })
      .catch((errorValue) => {
        if (mounted.current && generation === clientGeneration.current)
          setError(resourceMessage(errorValue));
      });
  }, [client, initial.id, clientGeneration, mounted]);
  const save = () =>
    void run(async (generation) => {
      await client.save(item.id, !item.saved);
      if (!mounted.current || generation !== clientGeneration.current) return;
      const updated = await client.detail(item.id);
      if (mounted.current && generation === clientGeneration.current) setItem(updated);
    });
  const apply = () =>
    void run(async (generation) => {
      const result = await client.apply(item.id, item.revision);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setNotice(`已导入云端词库，新增或更新 ${result.imported} 个词条。`);
    });
  const applyLocal = () =>
    void run(async (generation) => {
      if (!localDictionary) return;
      const result = await installCommunityDictionaryLocally(localDictionary, item);
      if (!mounted.current || generation !== clientGeneration.current || !result) return;
      setNotice(
        result.kind === "collection"
          ? `已添加为词库「${item.name}」，可以在「词库」里停用或删除。`
          : `已导入本机词库，应用 ${result.applied} 个词条。`,
      );
    });
  const storeReply = () =>
    void run(async (generation) => {
      if (!client.storeReply) return;
      await client.save(item.id, true);
      if (!mounted.current || generation !== clientGeneration.current) return;
      const latest = await client.detail(item.id);
      if (!mounted.current || generation !== clientGeneration.current) return;
      await client.storeReply(latest);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setItem(latest);
      setNotice("已添加到高情商回复键盘；只有点按生成时才会发送文字。");
    });
  const rateResource = (stars: number) =>
    void run(async (generation) => {
      await client.rate(item.id, stars);
      if (!mounted.current || generation !== clientGeneration.current) return;
      const updated = await client.detail(item.id);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setItem(updated);
      setNotice(`已评分：${stars} 星。`);
    });
  const removeReply = () =>
    void run(async (generation) => {
      if (!client.removeReply) return;
      await client.removeReply(item.id);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setNotice("已从本机高情商回复键盘移除，社区收藏保留。");
    });
  const unpublish = () =>
    void run(async (generation) => {
      await client.unpublish(item.id);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setConfirmDelete(false);
      onClose();
    });
  const report = async (reason: CommunityReportReason, detail: string) => {
    if (!client.report) return false;
    let reported = false;
    await run(async (generation) => {
      await client.report!(item.kind, item.id, reason, detail);
      if (!mounted.current || generation !== clientGeneration.current) return;
      reported = true;
      setNotice(communityReportedNotice);
    });
    return reported;
  };
  const onPublished = async () => {
    setEditing(false);
    const updated = await client.detail(item.id);
    if (!mounted.current || renderGeneration !== clientGeneration.current) return;
    setItem(updated);
  };

  return {
    item,
    error,
    notice,
    editing,
    confirmDelete,
    busy,
    save,
    apply,
    applyLocal,
    storeReply,
    rateResource,
    removeReply,
    unpublish,
    report,
    openEditor: () => setEditing(true),
    closeEditor: () => setEditing(false),
    requestUnpublish: () => setConfirmDelete(true),
    cancelUnpublish: () => setConfirmDelete(false),
    onPublished,
  };
}
