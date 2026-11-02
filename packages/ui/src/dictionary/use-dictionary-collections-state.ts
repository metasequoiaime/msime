import { useCallback, useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useAsyncGeneration } from "../settings/use-async-generation";
import type { DictionaryClient } from "./dictionary-contracts";
import {
  collectionFailureMessage,
  type DictionaryCollectionsClient,
  type DictionaryCollectionsView,
} from "./dictionary-collections";

export interface UseDictionaryCollectionsStateOptions {
  collections?: DictionaryCollectionsClient;
  dictionary?: Pick<DictionaryClient, "count">;
  /** 社区能力仅用作安装准备阶段的归属，不参与列表刷新。 */
  community?: object;
  galleryOpen: boolean;
  onNotice: (message: string) => void;
}

/** 词库集合列表、内置计数与修改的状态归属，不依赖设置表单或提示容器。 */
export function useDictionaryCollectionsState({
  collections,
  dictionary,
  community,
  galleryOpen,
  onNotice,
}: UseDictionaryCollectionsStateOptions) {
  const [view, setView] = useState<DictionaryCollectionsView | null>(null);
  const [loadFailure, setLoadFailure] = useState("");
  const [builtinCount, setBuiltinCount] = useState<number | null>(null);
  const [cantoneseCount, setCantoneseCount] = useState<number | null>(null);
  const reportFailure = useCallback(
    (message: string) => {
      if (message) onNotice(message);
    },
    [onNotice],
  );
  const runner = useAsyncActionRunner(reportFailure, undefined, collections, dictionary, community);
  const { busy, mounted } = runner;
  const reloadGeneration = useAsyncGeneration(collections, dictionary, galleryOpen);

  const reload = async (flush: boolean) => {
    const current = ++reloadGeneration.current;
    if (dictionary?.count) {
      dictionary
        .count("pinyin")
        .then(
          (count) =>
            mounted.current && current === reloadGeneration.current && setBuiltinCount(count),
        )
        .catch(() => undefined);
      dictionary
        .count("cantonese")
        .then(
          (count) =>
            mounted.current && current === reloadGeneration.current && setCantoneseCount(count),
        )
        .catch(() => undefined);
    }
    if (!collections) return;
    try {
      // 读列表时顺手把还没送出的增删再送一批，与 Android 打开词库页时相同。
      const next = flush ? await collections.flush() : await collections.load();
      if (!mounted.current || current !== reloadGeneration.current) return;
      setView(next);
      setLoadFailure("");
    } catch (error) {
      if (mounted.current && current === reloadGeneration.current)
        setLoadFailure(collectionFailureMessage(error));
    }
  };
  // 关闭社区图库后读回新安装的词库，图库打开期间不自动刷新列表。
  useEffect(() => {
    if (galleryOpen) return;
    void reload(true);
  }, [collections, dictionary, galleryOpen]);

  const captureOwner = () => {
    const current = runner.generation.current;
    return () => mounted.current && current === runner.generation.current;
  };

  const run = async (
    action: (isCurrent: () => boolean) => Promise<DictionaryCollectionsView | undefined>,
    done?: string | (() => string),
  ) => {
    const isCurrentOwner = captureOwner();
    let next: DictionaryCollectionsView | undefined;
    await runner.run(
      async (isCurrent) => {
        // 修改取得下一份列表的所有权，较早的刷新不能在回包时覆盖它。
        reloadGeneration.current++;
        const result = await action(isCurrent);
        if (!result || !isCurrent()) return;
        next = result;
        setView(result);
        if (done) onNotice(typeof done === "function" ? done() : done);
      },
      {
        formatError: collectionFailureMessage,
        onError: () => {
          if (collections) void reload(false);
        },
      },
    );
    return isCurrentOwner() ? next : undefined;
  };

  return { view, loadFailure, builtinCount, cantoneseCount, busy, run, captureOwner };
}

export type DictionaryCollectionsState = ReturnType<typeof useDictionaryCollectionsState>;
