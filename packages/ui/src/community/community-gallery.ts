import { useCallback, useEffect, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
import { appendUniqueById } from "./community-helpers";

export type CommunityGalleryPage<T> = { items: T[]; has_more: boolean };

export interface CommunityGalleryClient<T extends { id: string }> {
  list(offset: number, search: string, mine?: boolean): Promise<CommunityGalleryPage<T>>;
  detail(id: string): Promise<T>;
  rate(id: string, stars: number): Promise<void>;
  unpublish(id: string): Promise<void>;
}

export interface CommunityGalleryOptions<T extends { id: string }> {
  client: CommunityGalleryClient<T>;
  initialMine?: boolean;
  errorMessage?: (failure: unknown) => string;
  needsSignIn?: (failure: unknown) => boolean;
}

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
  const mounted = useRef(true);
  const clientGeneration = useRef(0);

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
        const page = await client.list(offset, query, mine);
        if (generation !== listGeneration.current) return true;
        setItems((current) => (append ? appendUniqueById(current, page.items) : page.items));
        nextOffset.current = offset + page.items.length;
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
    const currentClient = ++clientGeneration.current;
    mounted.current = true;
    setActionBusy(false);
    void requestList("", false, activeMine.current);
    return () => {
      mounted.current = false;
      if (clientGeneration.current === currentClient) clientGeneration.current++;
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

  const beginAction = useCallback(() => {
    if (!selected || actionBusy) return null;
    setActionBusy(true);
    return clientGeneration.current;
  }, [actionBusy, selected]);

  const isCurrent = useCallback(
    (generation: number) => mounted.current && generation === clientGeneration.current,
    [],
  );

  const endAction = useCallback(
    (generation: number) => {
      if (isCurrent(generation)) setActionBusy(false);
    },
    [isCurrent],
  );

  const runAction = useCallback(
    (action: (generation: number) => Promise<void>) => {
      const generation = clientGeneration.current;
      setSignInRequired(false);
      return runAsyncAction(
        {
          busy: actionBusy,
          isCurrent: () => isCurrent(generation),
          setBusy: setActionBusy,
          setError,
        },
        () => action(generation),
        {
          formatError: errorMessage,
          onError: (failure) => setSignInRequired(needsSignIn(failure)),
        },
      );
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
    setError,
    setActionNotice,
    setMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail,
    rateSelected,
    unpublishSelected,
    beginAction,
    isCurrent,
    endAction,
  };
}
