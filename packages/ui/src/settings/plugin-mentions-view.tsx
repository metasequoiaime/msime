import type { Dispatch, SetStateAction } from "react";
import { GroupList } from "../core/platform-controls";
import * as settings from "./settings-style";
import { PluginViewHeader } from "./plugin-view-header";
import type { MentionEntry } from "./plugin-types";

/** `client-core::plugins::mentions::MAX_ENTRIES`. */
export const MAX_MENTIONS = 1000;
/** `mentions::MAX_TEXT_UTF16`: the Windows candidate pipe's text field. */
const MAX_MENTION_TEXT_UTF16 = 199;
/** `mentions::MAX_KEY_BYTES`. */
const MAX_MENTION_KEY_BYTES = 64;
const MENTION_KEY = /^[a-z]+(?:'[a-z]+)*$/;

/**
 * Why the list cannot be saved as it stands, in the words the page shows, or null when it can. The rules are `mentions::validate_entry`'s, checked here so the user sees which row to fix before anything is sent; the host checks again.
 */
export function mentionListIssue(entries: readonly MentionEntry[]): string | null {
  if (entries.length > MAX_MENTIONS) return `名单最多 ${MAX_MENTIONS} 条。`;
  const seen = new Set<string>();
  for (const [index, entry] of entries.entries()) {
    const row = `第 ${index + 1} 行`;
    if (entry.text.trim().length === 0) return `${row}还没有填写名字或地点。`;
    // UTF-16 code units, which is what String.length counts.
    if (entry.text.length > MAX_MENTION_TEXT_UTF16) return `${row}太长了。`;
    if (/\p{Cc}/u.test(entry.text)) return `${row}含有控制字符。`;
    if (
      entry.key.length > 0 &&
      (entry.key.length > MAX_MENTION_KEY_BYTES || !MENTION_KEY.test(entry.key))
    ) {
      return `${row}的拼音只能是小写字母，音节之间用 ' 分隔，例如 zhang'san。`;
    }
    // Compared as saved: `saveMentions` trims each name before the host checks for duplicates.
    const text = entry.text.trim();
    if (seen.has(text)) return `「${text}」重复了。`;
    seen.add(text);
  }
  return null;
}

/** The @ mode's name list. Its rows live in `PluginsSection`, so edits survive leaving this view until they are saved. */
export function PluginMentionsView({
  mentions,
  setMentions,
  issue,
  dirty,
  working,
  onSave,
  onBack,
}: {
  mentions: MentionEntry[];
  setMentions: Dispatch<SetStateAction<MentionEntry[]>>;
  issue: string | null;
  dirty: boolean;
  working: boolean;
  onSave: () => void;
  onBack: () => void;
}) {
  const updateMention = (index: number, patch: Partial<MentionEntry>) =>
    setMentions((current) =>
      current.map((entry, position) => (position === index ? { ...entry, ...patch } : entry)),
    );
  return (
    <>
      <PluginViewHeader title="@ 名单" onBack={onBack} />
      <GroupList>
        <div className={settings.managerBlock}>
          <p className={settings.managerNote}>
            在「输入 → 实用功能」打开 @ 名字与地点后，按 @
            再输入拼音或首字母，就会从这份名单里出候选。名单只保存在本机，不随账号同步，也不会读取通讯录或位置。拼音可以留空，中文名字会自动取读音。
          </p>
          {mentions.map((entry, index) => (
            <div className={settings.phraseForm} key={index}>
              <label className={settings.field}>
                名字或地点
                <input
                  className={settings.fieldInput}
                  value={entry.text}
                  maxLength={MAX_MENTION_TEXT_UTF16}
                  onChange={(event) => updateMention(index, { text: event.target.value })}
                />
              </label>
              <label className={settings.field}>
                拼音
                <input
                  className={settings.fieldInput}
                  value={entry.key}
                  placeholder="zhang'san"
                  maxLength={MAX_MENTION_KEY_BYTES}
                  autoCapitalize="off"
                  spellCheck={false}
                  onChange={(event) => updateMention(index, { key: event.target.value })}
                />
              </label>
              <button
                type="button"
                className="secondary"
                aria-label={`删除第 ${index + 1} 行`}
                onClick={() =>
                  setMentions((current) => current.filter((_, position) => position !== index))
                }
              >
                删除
              </button>
            </div>
          ))}
          {issue && dirty && (
            <p className={settings.settingsWarning} role="alert">
              {issue}
            </p>
          )}
          <div className={settings.managerActions}>
            <button
              type="button"
              className="secondary"
              disabled={mentions.length >= MAX_MENTIONS}
              onClick={() => setMentions((current) => [...current, { text: "", key: "" }])}
            >
              添加
            </button>
            <button
              type="button"
              className="primary"
              disabled={working || !dirty || issue !== null}
              onClick={onSave}
            >
              保存名单
            </button>
          </div>
        </div>
      </GroupList>
    </>
  );
}
