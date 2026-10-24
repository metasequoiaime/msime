import { useId, useState } from "react";
import { useConfirm } from "../core/confirm";
import { formatZhNumber } from "../core/format-number";
import { GroupList, NavRow } from "../core/platform-controls";
import * as controls from "../core/platform-controls-style";
import { useToast } from "../core/toast";
import { SwitchRow } from "../settings/switch-row";
import * as settings from "../settings/settings-style";
import {
  displayPinyinCode,
  type DictionaryCollection,
  type DictionaryCollectionsView,
} from "./dictionary-collections";
import { DictionaryWordDialog } from "./dictionary-edit-dialogs";
import { Badge, PlusGlyph } from "./dictionary-detail-parts";
import type { DictionaryClient } from "./dictionary-contracts";
import { useBuiltinDictionaryEntries } from "./use-builtin-dictionary-entries";
import { SettingsSubViewHeader } from "../settings/settings-subview-header";

/** 删除这类危险操作：一行红字，没有图标和箭头。 */
function DangerRow({
  title,
  disabled,
  onClick,
}: {
  title: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={`${controls.navRow} ${controls.navRowButton}`}
      disabled={disabled}
      onClick={onClick}
    >
      {/* 颜色给到文字上：行本身的样式固定用正文色。 */}
      <span
        className="flex min-h-[52px] min-w-0 flex-1 items-center text-[16px] text-danger"
        data-row-title=""
      >
        {title}
      </span>
    </button>
  );
}

/** 一个命名词库：启用开关、条数、添加词条和删除。集合接口不列出单个集合的词条，所以和 Android 一样只显示条数。 */
export function CollectionDetail({
  collection,
  busy,
  onEnabled,
  onAddWord,
  onDelete,
}: {
  collection: DictionaryCollection;
  busy: boolean;
  onEnabled: (enabled: boolean) => void;
  onAddWord: (word: string, code: string) => Promise<DictionaryCollectionsView | undefined>;
  onDelete: () => Promise<void>;
}) {
  const [adding, setAdding] = useState(false);
  const { confirm, confirmation } = useConfirm();
  const pending =
    collection.pending > 0 ? `，还有 ${formatZhNumber(collection.pending)} 条等键盘同步` : "";
  return (
    <>
      <SettingsSubViewHeader title={collection.name} />
      <GroupList>
        <SwitchRow
          title="启用此词库"
          description={`${formatZhNumber(collection.entry_count)} 条${pending}`}
          checked={collection.enabled}
          disabled={busy}
          onChange={onEnabled}
        />
      </GroupList>
      <GroupList>
        <NavRow
          variant="me"
          icon={
            <Badge>
              <PlusGlyph />
            </Badge>
          }
          title="添加词条"
          disabled={busy}
          onClick={() => setAdding(true)}
        />
      </GroupList>
      <GroupList>
        <DangerRow
          title="删除此词库"
          disabled={busy}
          onClick={async () => {
            const confirmed = await confirm({
              title: `删除「${collection.name}」？`,
              message: "这个词库带来的词会从候选里去掉，你自己加过的词会留下。",
              confirmLabel: "删除",
              danger: true,
            });
            if (confirmed) await onDelete();
          }}
        />
      </GroupList>
      {adding && (
        <DictionaryWordDialog
          onClose={() => setAdding(false)}
          onSubmit={async (word, code) => {
            if (await onAddWord(word, code)) setAdding(false);
          }}
        />
      )}
      {confirmation}
    </>
  );
}

/** 内置拼音词库：始终启用，可以搜词条、加词和导出自己加过的词。 */
export function BuiltinDictionaryDetail({
  dictionary,
  count,
  onExport,
}: {
  dictionary: DictionaryClient;
  count: number | null;
  onExport?: () => Promise<string>;
}) {
  const toast = useToast();
  const { query, setQuery, entries, more, loading, reload, loadMore } = useBuiltinDictionaryEntries(
    { dictionary, onNotice: toast },
  );
  const [adding, setAdding] = useState(false);
  const searchId = useId();

  return (
    <>
      <SettingsSubViewHeader title="拼音词库" />
      <GroupList>
        <SwitchRow
          title="启用此词库"
          description={`内置词库始终启用${count ? `，有 ${formatZhNumber(count)} 条自己的词` : ""}`}
          checked
          disabled
          onChange={() => undefined}
        />
      </GroupList>
      <label htmlFor={searchId} className="sr-only">
        搜索词条
      </label>
      <input
        id={searchId}
        type="search"
        className="box-border h-10 w-full rounded-full border-0 px-4 text-[15px] [background:var(--p-group-bg)] [color:var(--p-text)]"
        placeholder="搜索词条"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
      />
      {/* 与 Android 的词库详情页一样：词条和「添加词条」在同一张卡片里，导出单独一张。 */}
      <GroupList>
        {entries.map((entry) => (
          <div
            key={`${entry.key}\u0000${entry.value}`}
            className="flex min-h-[48px] items-center gap-3 px-3.5 py-2"
          >
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[16px] [color:var(--p-text)]">
                {entry.value}
              </span>
              <span className="block truncate text-[12px] [color:var(--p-sub)]">
                {displayPinyinCode(entry.key)}
              </span>
            </span>
            <span className="shrink-0 text-[13px] [color:var(--p-sub)]">{entry.weight}</span>
          </div>
        ))}
        {!loading && entries.length === 0 && (
          <p className={settings.groupNote}>
            {query ? "没有匹配的词条。" : "还没有自己添加或学到的词。输入拼音可以搜索内置词条。"}
          </p>
        )}
        {more && (
          <NavRow
            variant="me"
            title={loading ? "正在读取…" : "加载更多"}
            disabled={loading}
            onClick={() => void loadMore()}
          />
        )}
        <NavRow
          variant="me"
          icon={
            <Badge>
              <PlusGlyph />
            </Badge>
          }
          title="添加词条"
          onClick={() => setAdding(true)}
        />
      </GroupList>
      {onExport && (
        <GroupList>
          <NavRow
            variant="me"
            title="导出词库"
            description="导出自己加过的拼音词条"
            onClick={() => void onExport().then((message) => message && toast(message))}
          />
        </GroupList>
      )}
      {adding && (
        <DictionaryWordDialog
          onClose={() => setAdding(false)}
          onSubmit={async (word, code) => {
            try {
              await dictionary.edit(
                null,
                { kind: "pinyin", key: code, value: word, weight: 10 },
                `phone-dictionary-${Date.now()}`,
              );
              setAdding(false);
              toast("已添加");
              void reload();
            } catch {
              toast("添加失败，请稍后重试。");
            }
          }}
        />
      )}
    </>
  );
}
