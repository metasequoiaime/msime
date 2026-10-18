import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { trackAsyncSubscription } from "../core/async-subscription";
import { isComposingKeyEvent } from "../core/ime-key-event";
import { readOptionalJson, writeOptionalJson } from "../core/optional-storage";
import { useTimedValue } from "../core/use-timed-value";
import { subscribeToInterval } from "../core/use-interval";
import { subscribeToDocumentVisibilityChange } from "../core/window-focus";
import {
  fallbackEmojiGroups,
  fallbackKaomojiGroups,
  fallbackSymbolGroups,
  type EmojiCatalogGroup,
  type EmojiCatalogItem,
} from "../emoji/emoji-catalog";
import { useEmojiNavigation } from "../emoji/use-emoji-navigation";
import { useAsyncGeneration } from "../settings/use-async-generation";
import type { EmojiPanelClient } from "./panel-client";
import type { PanelActivationMode } from "./panel-activation-buttons";
import {
  flattenGroups,
  matchesEmojiItem,
  matchingGroupItems,
  symbolCategoryTitle,
} from "./emoji-panel-helpers";
import { usePanelAction } from "./use-panel-action";

export type EmojiPage = "home" | "emoji" | "sticker" | "gif" | "kaomoji" | "symbols" | "clipboard";

/** 表情、符号和剪贴板面板的目录、操作与订阅状态；导航绑定须挂在面板根元素上。 */
export function useEmojiPanel(
  client: EmojiPanelClient,
  initialPage: "home" | "clipboard" = "home",
) {
  const [page, setPage] = useState<EmojiPage>(initialPage);
  const [itemPage, setItemPage] = useState(0);
  const itemsPerPage = 48;
  const [query, setQuery] = useState("");
  const [categories, setCategories] = useState({ emoji: "all", symbols: "all" });
  const [recent, setRecent] = useState<EmojiCatalogItem[]>(() => {
    const value = readOptionalJson("msime.emoji.recent");
    return Array.isArray(value)
      ? (value
          .filter(
            (item) => item && typeof item.text === "string" && typeof item.keywords === "string",
          )
          .slice(0, 28) as EmojiCatalogItem[])
      : [];
  });
  const [symbolRecent, setSymbolRecent] = useState<EmojiCatalogItem[]>(() => {
    const value = readOptionalJson("msime.symbols.recent");
    return Array.isArray(value)
      ? value
          .filter(
            (item) => item && typeof item.text === "string" && typeof item.keywords === "string",
          )
          .slice(0, 28)
      : [];
  });
  useEffect(() => {
    writeOptionalJson("msime.symbols.recent", symbolRecent);
  }, [symbolRecent]);
  const [clipboard, setClipboard] = useState<string[]>([]);
  const {
    busy: clipboardBusy,
    busyRef: clipboardMutation,
    revisionRef: operationRevision,
    run: runOperation,
    invalidate,
  } = usePanelAction(setNotice);
  const [clipboardLoadFailed, setClipboardLoadFailed] = useState(false);
  const [clipboardRefresh, setClipboardRefresh] = useState(0);
  const [activationMode, setActivationMode] = useState<PanelActivationMode>("copy");
  const [clipboardEnabled, setClipboardEnabled] = useState<boolean | null>(null);
  const clipboardGeneration = useAsyncGeneration();
  const deletedRowFocus = useRef<{ element: HTMLElement; index: number; query: string } | null>(
    null,
  );
  const [notice, setNoticeText] = useTimedValue("");
  const [catalog, setCatalog] = useState({
    emoji: fallbackEmojiGroups,
    kaomoji: fallbackKaomojiGroups,
    symbols: fallbackSymbolGroups,
  });
  const [catalogUnavailable, setCatalogUnavailable] = useState<("emoji" | "kaomoji" | "symbols")[]>(
    [],
  );
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [catalogRetry, setCatalogRetry] = useState(0);
  const catalogGeneration = useAsyncGeneration(client, catalogRetry);
  const clipboardLifecycle = useAsyncGeneration(client, page, clipboardRefresh);

  function setNotice(message: string, temporary = false) {
    setNoticeText(message, temporary ? 1600 : undefined);
  }

  useEffect(() => {
    clipboardMutation.current = false;
    deletedRowFocus.current = null;
    setNotice("");
    return invalidate;
  }, [client, invalidate]);

  useEffect(() => {
    writeOptionalJson("msime.emoji.recent", recent.slice(0, 28));
  }, [recent]);

  useEffect(() => {
    if (!client.rememberInputTarget) return;
    void client.rememberInputTarget().catch(() => setNotice("未能记录前台输入窗口"));
  }, [client]);

  useEffect(() => {
    setCatalog({
      emoji: fallbackEmojiGroups,
      kaomoji: fallbackKaomojiGroups,
      symbols: fallbackSymbolGroups,
    });
    setCatalogUnavailable([]);
  }, [client]);

  useEffect(() => {
    if (!client.loadCatalog) {
      setCatalogLoading(false);
      return;
    }
    const generation = catalogGeneration.current;
    setCatalogLoading(true);
    void Promise.resolve()
      .then(() => client.loadCatalog!())
      .then((next) => {
        if (generation !== catalogGeneration.current) return;
        const unavailable = next.unavailable ?? [];
        setCatalog((current) => ({
          emoji: unavailable.includes("emoji") ? current.emoji : next.emoji,
          kaomoji: unavailable.includes("kaomoji") ? current.kaomoji : next.kaomoji,
          symbols: unavailable.includes("symbols") ? current.symbols : next.symbols,
        }));
        setCatalogUnavailable(unavailable);
      })
      .catch(() => {
        if (generation === catalogGeneration.current)
          setCatalogUnavailable(["emoji", "kaomoji", "symbols"]);
      })
      .finally(() => {
        if (generation === catalogGeneration.current) setCatalogLoading(false);
      });
  }, [client, catalogRetry, catalogGeneration]);

  useEffect(() => {
    if (!client.clipboard?.list) {
      setClipboardLoadFailed(false);
      return;
    }
    const generation = clipboardLifecycle.current;
    const isCurrent = () => generation === clipboardLifecycle.current;
    let stopPoll: (() => void) | undefined;
    let stopVisibility: (() => void) | undefined;
    let inFlight = 0;
    let refreshPending = false;
    const refresh = () => {
      if (!isCurrent()) return;
      if (inFlight !== 0) {
        refreshPending = true;
        // 新通知使当前快照失效；连续多个通知只需合并为一次后续读取。
        ++clipboardGeneration.current;
        return;
      }
      inFlight++;
      const request = ++clipboardGeneration.current;
      void Promise.resolve()
        .then(() =>
          Promise.all([
            client.clipboard!.list!(),
            client.clipboard?.isEnabled?.() ?? Promise.resolve(true),
          ]),
        )
        .then(([value, enabled]) => {
          if (isCurrent() && request === clipboardGeneration.current) {
            setClipboard(enabled ? value : []);
            setClipboardEnabled(enabled);
            setClipboardLoadFailed(false);
          }
        })
        .catch(() => {
          if (isCurrent() && request === clipboardGeneration.current) {
            setClipboard([]);
            setClipboardEnabled(null);
            setClipboardLoadFailed(true);
          }
        })
        .finally(() => {
          inFlight--;
          if (isCurrent() && refreshPending) {
            refreshPending = false;
            refresh();
          }
        });
    };
    const pollVisible = () => {
      if (
        isCurrent() &&
        document.visibilityState === "visible" &&
        !clipboardMutation.current &&
        inFlight === 0
      )
        refresh();
    };
    const start = async () => {
      let stop: (() => void) | undefined;
      try {
        stop = await client.clipboard?.onChanged?.(refresh);
      } catch {
        /* 宿主通知注册失败时继续轮询。 */
      }
      if (!isCurrent()) return stop;
      // 注册后补读，覆盖初始快照与订阅就绪之间的变化。
      refresh();
      if (stop && stopPoll !== undefined) {
        stopPoll();
        stopPoll = undefined;
        stopVisibility?.();
      }
      return stop;
    };
    // 首次读取不等待通知注册；订阅就绪前沿用 Windows 的 400 ms 轮询间隔。
    refresh();
    if (page === "clipboard") {
      stopPoll = subscribeToInterval(pollVisible, 400);
      stopVisibility = subscribeToDocumentVisibilityChange(pollVisible);
    }
    const disposeSubscription = trackAsyncSubscription(start(), isCurrent);
    return () => {
      refreshPending = false;
      ++clipboardGeneration.current;
      stopPoll?.();
      stopVisibility?.();
      disposeSubscription();
    };
  }, [client, page, clipboardLifecycle, clipboardRefresh]);

  type DisplayGroup = EmojiCatalogGroup & { moreTarget?: EmojiPage; flow?: boolean };
  const groups =
    page === "emoji" ? catalog.emoji : page === "kaomoji" ? catalog.kaomoji : catalog.symbols;
  const categoryPage = page === "emoji" || page === "symbols" ? page : null;
  // 插件组的分类键带上包 id：同名的内置分类或别的包不会和它合并。
  const categoryKey = (group: EmojiCatalogGroup) =>
    group.pack !== undefined
      ? JSON.stringify(["pack", group.pack])
      : page === "symbols" && group.parent
        ? `parent:${group.parent}`
        : `group:${group.title}`;
  const categoryTabs: { id: string; title: string; icon: string }[] = [];
  for (const group of groups) {
    const id = categoryKey(group);
    if (!categoryTabs.some((tab) => tab.id === id)) {
      categoryTabs.push({
        id,
        title: page === "symbols" ? group.parent || group.title : group.title,
        icon: group.icon,
      });
    }
  }
  const selectedCategory = categoryPage ? categories[categoryPage] : "all";
  const activeCategory =
    selectedCategory === "recent" || categoryTabs.some((tab) => tab.id === selectedCategory)
      ? selectedCategory
      : "all";
  const recentGroup: EmojiCatalogGroup = { title: "最近使用", icon: "◷", items: recent };
  const selectedGroups =
    categoryPage && (page === "emoji" || !query)
      ? activeCategory === "recent"
        ? [recentGroup]
        : activeCategory === "all"
          ? groups
          : groups.filter((group) => categoryKey(group) === activeCategory)
      : groups;
  const filteredGroups: DisplayGroup[] = selectedGroups
    .map((group) => ({
      ...group,
      flow: page === "kaomoji",
      items: matchingGroupItems(
        page === "symbols" && !group.pack
          ? {
              ...group,
              keywords: [
                group.keywords,
                symbolCategoryTitle(group.title),
                symbolCategoryTitle(group.parent ?? ""),
              ].join(" "),
            }
          : group,
        query,
      ),
    }))
    .filter((group) => group.items.length);
  const symbolPreview = query
    ? catalog.symbols.flatMap((group) => matchingGroupItems(group, query)).slice(0, 18)
    : catalog.symbols
        .flatMap((group) => group.items.map((item, index) => ({ item, index })))
        .sort((left, right) => left.index - right.index)
        .slice(0, 18)
        .map(({ item }) => item);
  const homeGroups: DisplayGroup[] = [
    { ...recentGroup, items: recent.filter((item) => matchesEmojiItem(item, query)) },
    {
      title: "Emoji",
      icon: catalog.emoji[0]?.icon ?? "😀",
      moreTarget: "emoji",
      items: flattenGroups(catalog.emoji)
        .filter((item) => matchesEmojiItem(item, query))
        .slice(0, 18),
    },
    {
      title: "Kaomoji",
      icon: catalog.kaomoji[0]?.icon ?? "ヾ",
      moreTarget: "kaomoji",
      flow: true,
      items: catalog.kaomoji.flatMap((group) => matchingGroupItems(group, query)).slice(0, 15),
    },
    {
      title: "Symbols",
      icon: catalog.symbols[0]?.icon ?? "★",
      moreTarget: "symbols",
      items: symbolPreview,
    },
  ];

  function selectCategory(next: string) {
    if (!categoryPage) return;
    setCategories((current) => ({ ...current, [categoryPage]: next }));
    setItemPage(0);
  }

  const visibleClipboard = clipboard.filter((item) =>
    item.toLocaleLowerCase().includes(query.toLocaleLowerCase()),
  );
  const canCopy = Boolean(client.copyText ?? client.clipboard?.copy);
  const effectiveMode = !client.sendText ? "copy" : !canCopy ? "input" : activationMode;

  function copy(text: string, isClipboardItem = false, keywords = text) {
    const input = !isClipboardItem && effectiveMode === "input";
    const action = input
      ? client.sendText
      : isClipboardItem
        ? (client.clipboard?.copy ?? client.copyText)
        : (client.copyText ?? client.clipboard?.copy);
    if (!action) {
      setNotice("当前宿主未提供此操作");
      return;
    }
    const revision = ++operationRevision.current;
    void action(text)
      .then(() => {
        if (revision !== operationRevision.current) return;
        setNotice(
          input ? "已输入到原应用" : isClipboardItem ? "已复制到剪贴板" : `已复制：${text}`,
          true,
        );
        if (page === "symbols")
          setSymbolRecent((current) =>
            [{ text, keywords }, ...current.filter((item) => item.text !== text)].slice(0, 28),
          );
        if (!isClipboardItem)
          setRecent((current) =>
            [{ text, keywords }, ...current.filter((item) => item.text !== text)].slice(0, 28),
          );
      })
      .catch(() => {
        if (revision === operationRevision.current)
          setNotice(input ? "输入失败，可切换复制后手动粘贴" : "无法访问剪贴板");
      });
  }

  function changeClipboard(
    action: () => Promise<string[] | void>,
    fallback: () => string[],
    message: string,
  ) {
    return runOperation(async (revision) => {
      ++clipboardGeneration.current;
      const result = await action();
      if (revision !== operationRevision.current) return;
      const request = ++clipboardGeneration.current;
      try {
        const [next, enabled] = await Promise.all([
          client.clipboard?.list ? client.clipboard.list() : Promise.resolve(result ?? fallback()),
          client.clipboard?.isEnabled?.() ?? Promise.resolve(true),
        ]);
        if (revision !== operationRevision.current) return;
        if (request === clipboardGeneration.current) {
          setClipboard(enabled ? next : []);
          setClipboardEnabled(enabled);
          setClipboardLoadFailed(false);
        }
        setNotice(message, true);
      } catch {
        if (revision === operationRevision.current)
          setNotice(`${message}，但列表刷新失败，请重新打开剪贴板页`);
      }
    }, "无法更新剪贴板历史");
  }

  function enableClipboard() {
    if (client.clipboard?.enable)
      return changeClipboard(client.clipboard.enable, () => [], "剪贴板历史已开启");
  }

  function syncClipboard() {
    if (client.clipboard?.sync)
      return changeClipboard(client.clipboard.sync, () => clipboard, "剪贴板已同步");
  }

  function pasteClipboard(text: string) {
    const paste = client.clipboard?.paste;
    if (!paste) return;
    return runOperation(async (revision) => {
      await paste(text);
      if (revision === operationRevision.current) setNotice("已粘贴到原应用", true);
    }, "无法粘贴，请使用复制后手动粘贴");
  }

  function removeClipboard(text: string) {
    if (!client.clipboard?.remove || clipboardMutation.current) return;
    const panel = navigation.ref.current;
    const focused = panel?.ownerDocument.activeElement;
    const rows = Array.from(panel?.querySelectorAll<HTMLElement>("[data-clipboard-row]") ?? []);
    const index = rows.findIndex((row) => focused != null && row.contains(focused));
    if (focused instanceof HTMLElement && index >= 0)
      deletedRowFocus.current = { element: focused, index, query };
    return changeClipboard(
      () => client.clipboard!.remove!(text),
      () => clipboard.filter((item) => item !== text),
      "记录已删除",
    );
  }

  function clipboardItemKey(event: import("react").KeyboardEvent<HTMLButtonElement>, text: string) {
    if (
      event.defaultPrevented ||
      isComposingKeyEvent(event) ||
      event.altKey ||
      event.metaKey ||
      event.shiftKey ||
      clipboardMutation.current
    )
      return;
    const copyKey = event.ctrlKey && event.key.toLowerCase() === "c" && canCopy;
    const deleteKey = !event.ctrlKey && event.key === "Delete" && !!client.clipboard?.remove;
    if (!copyKey && !deleteKey) return;
    event.preventDefault();
    if (event.repeat) return;
    if (deleteKey) void removeClipboard(text);
    else void copy(text, true);
  }
  function clearClipboard() {
    if (client.clipboard?.clear)
      return changeClipboard(client.clipboard.clear, () => [], "剪贴板历史已清空");
  }

  function selectPage(next: EmojiPage) {
    if (next === "symbols") setActivationMode("input");
    setPage(next);
    setItemPage(0);
    setQuery("");
    setNotice(
      next === "clipboard" || effectiveMode === "copy"
        ? "点击项目即可复制"
        : "点击项目即可输入到原应用",
    );
  }

  const isDetail = page !== "home";
  const sourceGroups =
    page === "home" ? homeGroups.filter((group) => group.items.length) : filteredGroups;
  const seenDisplayItems = new Set<string>();
  const displayGroups = sourceGroups
    .map((group) => ({
      ...group,
      items: group.items
        .filter((item) => {
          if (seenDisplayItems.has(item.text)) return false;
          seenDisplayItems.add(item.text);
          return true;
        })
        .slice(itemPage * itemsPerPage, (itemPage + 1) * itemsPerPage),
    }))
    .filter((group) => group.items.length);
  const itemPageCount = Math.max(
    1,
    Math.max(
      ...(page === "home" ? homeGroups : filteredGroups).map((group) =>
        Math.ceil(group.items.length / itemsPerPage),
      ),
      1,
    ),
  );
  function clearRecent() {
    if (clipboardMutation.current) return;
    setRecent([]);
    setNotice("最近使用已清除", true);
  }
  function closeEmoji() {
    return runOperation(async () => {
      await client.close();
    }, "无法关闭面板，请重试");
  }
  const navigation = useEmojiNavigation(
    () => {
      if (page !== "home") {
        selectPage("home");
        navigation.ref.current
          ?.querySelector<HTMLInputElement>("input[data-panel-search]")
          ?.focus();
      } else {
        void closeEmoji();
      }
    },
    JSON.stringify([page, query, activeCategory]),
  );
  useLayoutEffect(() => {
    const pending = deletedRowFocus.current;
    if (!pending) return;
    if (page !== "clipboard" || query !== pending.query) {
      deletedRowFocus.current = null;
      return;
    }
    if (clipboardBusy) return;
    deletedRowFocus.current = null;
    const panel = navigation.ref.current;
    if (!panel || pending.element.isConnected) return;
    const focused = panel.ownerDocument.activeElement;
    if (focused && focused !== panel.ownerDocument.body && focused !== pending.element) return;
    const items = Array.from(
      panel.querySelectorAll<HTMLButtonElement>("[data-clipboard-item]:not(:disabled)"),
    );
    const target =
      items[Math.min(pending.index, items.length - 1)] ??
      panel.querySelector<HTMLInputElement>("input[data-panel-search]");
    target?.focus({ preventScroll: true });
    target?.scrollIntoView({ block: "nearest", inline: "nearest" });
  }, [clipboard, clipboardBusy, page, query]);
  function updateQuery(value: string) {
    setQuery(value);
    setItemPage(0);
    setNotice("");
  }
  function retryCatalog() {
    setCatalogLoading(true);
    setCatalogRetry((value) => value + 1);
  }
  function selectActivation(mode: PanelActivationMode) {
    setActivationMode(mode);
    setNotice(mode === "copy" ? "点击项目即可复制" : "点击项目即可输入到原应用");
  }
  function clearSymbolRecent() {
    setSymbolRecent([]);
  }
  function selectSymbolCategory(id: string) {
    setQuery("");
    selectCategory(id);
  }
  function refreshClipboard() {
    if (!clipboardMutation.current) setClipboardRefresh((value) => value + 1);
  }
  function showMore(target: EmojiPage) {
    setCategories((current) => ({ ...current, emoji: "all", symbols: "all" }));
    selectPage(target);
  }
  function previousItemPage() {
    setItemPage((value) => Math.max(0, value - 1));
  }
  function nextItemPage() {
    setItemPage((value) => Math.min(itemPageCount - 1, value + 1));
  }
  return {
    page,
    itemPage,
    query,
    recent,
    symbolRecent,
    clipboard,
    clipboardBusy,
    clipboardLoadFailed,
    clipboardEnabled,
    notice,
    catalogUnavailable,
    catalogLoading,
    categoryPage,
    categoryTabs,
    activeCategory,
    filteredGroups,
    visibleClipboard,
    canCopy,
    effectiveMode,
    isDetail,
    displayGroups,
    itemPageCount,
    navigation,
    selectPage,
    selectCategory,
    copy,
    clipboardItemKey,
    enableClipboard,
    syncClipboard,
    pasteClipboard,
    removeClipboard,
    clearClipboard,
    clearRecent,
    closeEmoji,
    updateQuery,
    retryCatalog,
    selectActivation,
    clearSymbolRecent,
    selectSymbolCategory,
    refreshClipboard,
    showMore,
    previousItemPage,
    nextItemPage,
  };
}

export type EmojiPanelState = ReturnType<typeof useEmojiPanel>;
