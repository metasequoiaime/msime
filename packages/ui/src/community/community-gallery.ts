import { useCallback, useEffect, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
import { appendUniqueById } from "./community-helpers";
import { communityReportedNotice, type CommunityReportReason } from "./community-report";
import { useCommunityClientLifecycle } from "./use-community-client-lifecycle";

export type CommunityGalleryPage<T> = {
  items: T[];
  has_more: boolean;
  /** Rows the server listed on this page that the host left out (a kind this client cannot install). They still count toward the next offset, so 加载更多 resumes after them instead of reading them again. */
  skipped?: number;
};

export interface CommunityGalleryClient<T extends { id: string }> {
  list(offset: number, search: string, mine?: boolean): Promise<CommunityGalleryPage<T>>;
  detail(id: string): Promise<T>;
  rate(id: string, stars: number): Promise<void>;
  unpublish(id: string): Promise<void>;
  /** Reports another user's item to the moderators; absent hides the 举报 entry. */
  report?(id: string, reason: CommunityReportReason, detail: string): Promise<void>;
}

export interface CommunityGalleryOptions<T extends { id: string }> {
  client: CommunityGalleryClient<T>;
  initialMine?: boolean;
  errorMessage?: (failure: unknown) => string;
  needsSignIn?: (failure: unknown) => boolean;
}

export interface CommunityGalleryActionOptions {
  clearNotice?: boolean;
  ignoreError?: (failure: unknown) => boolean;
  onError?: (failure: unknown) => void;
}

/** How many pages in a row that the host emptied (see `CommunityGalleryPage.skipped`) one request reads past before it stops and leaves the rest to 加载更多. */
const MAX_SKIPPED_PAGES = 5;

const defaultErrorMessage = (failure: unknown) =>
  failure instanceof Error ? failure.message : "加载失败，请稍后重试。";
const defaultNeedsSignIn = () => false;

export function useCommunityGallery<T extends { id: string }>({
  client,
  initialMine = false,
  errorMessage = defaultErrorMessage,
  needsSignIn = defaultNeedsSignIn,
}: CommunityGalleryOptions<T>) {
  const [items, setItems] = useState<T[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [listBusy, setListBusy] = useState(true);
  const [detailBusy, setDetailBusy] = useState(false);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<T | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [actionNotice, setActionNotice] = useState("");
  const [mineOnly, setMineOnly] = useState(initialMine);
  const [signInRequired, setSignInRequired] = useState(false);
  const [confirmUnpublish, setConfirmUnpublish] = useState(false);
  const listGeneration = useRef(0);
  const detailGeneration = useRef(0);
  const nextOffset = useRef(0);
  const activeSearch = useRef("");
  const activeMine = useRef(initialMine);
  const {
    mounted,
    clientGeneration,
    actionRunning: actionBusyRef,
    isCurrent,
  } = useCommunityClientLifecycle(client, errorMessage, needsSignIn);

  const fail = useCallback(
    (failure: unknown) => {
      if (!mounted.current) return;
      setError(errorMessage(failure));
      setSignInRequired(needsSignIn(failure));
    },
    [errorMessage, needsSignIn],
  );

  /** Resolves false only when this request was the latest and failed, which leaves the list as it was; true when it succeeded or a newer request superseded it. */
  const requestList = useCallback(
    async (query: string, append: boolean, mine = activeMine.current): Promise<boolean> => {
      const generation = ++listGeneration.current;
      const offset = append ? nextOffset.current : 0;
      setListBusy(true);
      setError("");
      setSignInRequired(false);
      try {
        let page = await client.list(offset, query, mine);
        let pageOffset = offset;
        // A page the host emptied by leaving out every row (all of a kind this client cannot install) shows nothing yet has more after it: read on, a few pages at most, so the list does not look empty while installable rows follow.
        for (
          let extra = 0;
          extra < MAX_SKIPPED_PAGES &&
          page.items.length === 0 &&
          page.has_more &&
          (page.skipped ?? 0) > 0;
          extra++
        ) {
          if (generation !== listGeneration.current) return true;
          pageOffset += page.skipped ?? 0;
          page = await client.list(pageOffset, query, mine);
        }
        if (generation !== listGeneration.current) return true;
        const items = page.items;
        setItems((current) => (append ? appendUniqueById(current, items) : items));
        nextOffset.current = pageOffset + page.items.length + (page.skipped ?? 0);
        if (!append) {
          activeSearch.current = query;
          activeMine.current = mine;
        }
        setHasMore(page.has_more);
        return true;
      } catch (requestError) {
        if (generation !== listGeneration.current) return true;
        fail(requestError);
        if (!append) setMineOnly(activeMine.current);
        return false;
      } finally {
        if (generation === listGeneration.current) setListBusy(false);
      }
    },
    [client, fail],
  );

  useEffect(() => {
    setActionBusy(false);
    void requestList("", false, activeMine.current);
    return () => {
      listGeneration.current += 1;
      detailGeneration.current += 1;
    };
  }, [client, requestList]);

  const open = useCallback(
    (item: T) => {
      const generation = ++detailGeneration.current;
      setSelected(item);
      setDetailBusy(true);
      setError("");
      setSignInRequired(false);
      setActionNotice("");
      setConfirmUnpublish(false);
      void client
        .detail(item.id)
        .then((value) => {
          if (generation === detailGeneration.current) setSelected(value);
        })
        .catch((detailError) => {
          if (generation === detailGeneration.current) fail(detailError);
        })
        .finally(() => {
          if (generation === detailGeneration.current) setDetailBusy(false);
        });
    },
    [client, fail],
  );

  const closeDetail = useCallback(() => {
    if (actionBusy) return;
    detailGeneration.current += 1;
    setSelected(null);
    setDetailBusy(false);
    setError("");
    setSignInRequired(false);
    setConfirmUnpublish(false);
  }, [actionBusy]);

  const replaceSelected = useCallback((updated: T) => {
    setSelected(updated);
    setItems((current) => current.map((item) => (item.id === updated.id ? updated : item)));
  }, []);

  const beginAction = useCallback(() => {
    if (!selected || actionBusy) return null;
    if (actionBusyRef.current) return null;
    actionBusyRef.current = true;
    setActionBusy(true);
    return clientGeneration.current;
  }, [actionBusy, selected]);

  const endAction = useCallback(
    (generation: number) => {
      if (isCurrent(generation)) setActionBusy(false);
      if (isCurrent(generation)) actionBusyRef.current = false;
    },
    [isCurrent],
  );

  const runAction = useCallback(
    (
      action: (generation: number) => Promise<void>,
      options: CommunityGalleryActionOptions = {},
    ) => {
      if (actionBusyRef.current || actionBusy) return Promise.resolve();
      const generation = clientGeneration.current;
      actionBusyRef.current = true;
      setSignInRequired(false);
      return runAsyncAction(
        {
          busy: actionBusy,
          isCurrent: () => isCurrent(generation),
          setBusy: setActionBusy,
          setError,
          setNotice: options.clearNotice ? setActionNotice : undefined,
        },
        () => action(generation),
        {
          formatError: errorMessage,
          ignoreError: options.ignoreError,
          onError: (failure) => {
            setSignInRequired(needsSignIn(failure));
            options.onError?.(failure);
          },
        },
      ).finally(() => {
        if (isCurrent(generation)) actionBusyRef.current = false;
      });
    },
    [actionBusy, errorMessage, isCurrent, needsSignIn],
  );

  const rateSelected = useCallback(
    async (stars: number) => {
      if (!selected || actionBusy) return;
      await runAction(async (generation) => {
        await client.rate(selected.id, stars);
        const updated = await client.detail(selected.id);
        if (!isCurrent(generation)) return;
        setSelected(updated);
        setActionNotice(`已评分：${stars} 星。`);
      });
    },
    [actionBusy, client, isCurrent, runAction, selected],
  );

  const unpublishSelected = useCallback(
    async (successMessage: string) => {
      if (!selected || actionBusy) return;
      await runAction(async (generation) => {
        await client.unpublish(selected.id);
        if (!isCurrent(generation)) return;
        detailGeneration.current += 1;
        setSelected(null);
        setConfirmUnpublish(false);
        setActionNotice(successMessage);
        await requestList(activeSearch.current, false, activeMine.current);
      });
    },
    [actionBusy, client, isCurrent, requestList, runAction, selected],
  );

  /** Resolves true once the report was accepted, so the form can close; a failure shows in the gallery's error like any other action. */
  const reportSelected = useCallback(
    async (reason: CommunityReportReason, detail: string) => {
      const report = client.report;
      if (!selected || actionBusy || !report) return false;
      let reported = false;
      await runAction(
        async (generation) => {
          await report(selected.id, reason, detail);
          if (!isCurrent(generation)) return;
          reported = true;
          setActionNotice(communityReportedNotice);
        },
        { clearNotice: true },
      );
      return reported;
    },
    [actionBusy, client, isCurrent, runAction, selected],
  );

  return {
    items,
    hasMore,
    listBusy,
    detailBusy,
    error,
    selected,
    actionBusy,
    actionNotice,
    mineOnly,
    signInRequired,
    confirmUnpublish,
    fail,
    activeSearch: activeSearch.current,
    setItems,
    setSelected,
    replaceSelected,
    setError,
    setActionNotice,
    setMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail,
    rateSelected,
    unpublishSelected,
    reportSelected,
    beginAction,
    isCurrent,
    endAction,
    runAction,
  };
}
