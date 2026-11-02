import { useEffect, useState } from "react";
import type { ConfirmRequest } from "../core/confirm";
import { GroupList } from "../core/platform-controls";
import { ControlledSelect } from "../core/controlled-select";
import * as settings from "./settings-style";
import { ActionButton } from "./action-button";
import { PluginNavRow } from "./plugin-nav-row";
import { PluginViewHeader } from "./plugin-view-header";
import { SettingsInputField } from "./settings-input-field";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsManagerNote } from "./settings-manager-note";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsPhraseForm } from "./settings-phrase-form";
import { SettingsTextareaField } from "./settings-textarea-field";
import { SettingsWarning } from "./settings-warning";
import { SettingsEmptyMessage } from "./settings-empty-message";
import { kindLabels, pluginErrorMessage } from "./plugin-catalog-helpers";
import {
  AUTHORED_KINDS,
  DEFAULT_LICENSE,
  MAX_COMMANDS,
  MAX_GROUPS,
  MAX_PHRASES,
  MAX_TEXT_UTF16,
  MAX_WORDS,
  appendRows,
  blankRow,
  draftFromPack,
  draftIssue,
  emptyCommand,
  emptyDraft,
  emptyGroup,
  emptyPhrase,
  emptyWord,
  packFromDraft,
  parsePhraseLines,
  parseWordLines,
  QUICK_PHRASE_CURSOR_NOTE,
  type AuthoringDraft,
  type DraftGroup,
  type DraftWord,
} from "./plugin-editor-model";
import type { AuthoredKind, AuthoredPack, PluginCommand, PluginPhrase } from "./plugin-types";
import { useMountedRef } from "./use-mounted-ref";

/** 长列表一次画出的行数：单词本可以有两万行，全部画出来会让页面卡住。 */
const ROW_PAGE = 100;

/** 介绍文本框：比提示词编辑框矮一些。 */
const descriptionInput = `${settings.fieldInput} min-h-16 w-full resize-y leading-relaxed`;
/** 批量粘贴和符号列表的文本框。 */
const listInput = `${settings.fieldInput} min-h-24 w-full resize-y leading-relaxed`;

export interface PluginEditorViewProps {
  /** 编辑已安装的包时是它的类型和 id；新建时为 null，先选类型。 */
  target: { kind: AuthoredKind; id: string } | null;
  /** 读出已安装的包：`PluginClient.loadAuthoredPack`。 */
  load: (kind: AuthoredKind, id: string) => Promise<AuthoredPack>;
  /** 保存并安装；失败时抛出宿主的 `{code, detail}`，由这里就地说明。 */
  onSave: (pack: AuthoredPack, replace: boolean) => Promise<void>;
  confirm: (request: ConfirmRequest) => Promise<boolean>;
  /** 新建时列出短语表和指令表；宿主不路由 K 和 / 模式时（`HostCapabilities.plugin_triggers` 为假的触屏插件页）为 false，那里做出来的表既不列出也用不上。缺省为 true。 */
  tables?: boolean;
  onBack: () => void;
}

/** 设置页的插件编辑器：选类型、填基本信息和内容，保存后由宿主按导入同样的规则校验并安装。草稿只在这个视图里，离开前有未保存的修改时先确认。 */
export function PluginEditorView({
  target,
  load,
  onSave,
  confirm,
  tables = true,
  onBack,
}: PluginEditorViewProps) {
  const mounted = useMountedRef();
  const [draft, setDraft] = useState<AuthoringDraft | null>(null);
  const [baseline, setBaseline] = useState<AuthoringDraft | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [shown, setShown] = useState(ROW_PAGE);
  const [bulk, setBulk] = useState("");
  const [bulkIssue, setBulkIssue] = useState<string | null>(null);

  useEffect(() => {
    if (!target) return;
    let current = true;
    load(target.kind, target.id)
      .then((pack) => {
        if (!current) return;
        const loaded = draftFromPack(pack);
        setDraft(loaded);
        setBaseline(loaded);
      })
      .catch((error: unknown) => {
        if (current) setLoadError(pluginErrorMessage(error, "无法读取这个插件，请重试。"));
      });
    return () => {
      current = false;
    };
    // 只按要编辑的包读一次；`load` 的身份随父组件重渲染变化，不应触发重读。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target?.kind, target?.id]);

  const dirty = draft !== null && JSON.stringify(draft) !== JSON.stringify(baseline);
  const issue = draft ? draftIssue(draft) : null;
  const editing = target !== null;

  const back = async () => {
    if (
      dirty &&
      !(await confirm({
        title: "放弃修改",
        message: "还没有保存的修改会丢掉。",
        confirmLabel: "放弃",
        danger: true,
      }))
    )
      return;
    onBack();
  };

  const pick = (kind: AuthoredKind) => {
    const fresh = emptyDraft(kind);
    setDraft(fresh);
    setBaseline(fresh);
  };

  const update = (patch: Partial<AuthoringDraft>) => {
    setSaveError(null);
    setDraft((current) => (current ? { ...current, ...patch } : current));
  };

  const save = async () => {
    if (!draft || issue || saving) return;
    setSaving(true);
    setSaveError(null);
    try {
      await onSave(packFromDraft(draft), draft.id !== null);
    } catch (error) {
      if (mounted.current) setSaveError(pluginErrorMessage(error, "保存失败，请重试。"));
    } finally {
      if (mounted.current) setSaving(false);
    }
  };

  const kindLabel = draft ? kindLabels[draft.kind] : "插件";
  const title = editing
    ? `编辑「${baseline?.name ?? target.id}」`
    : draft
      ? `新建${kindLabel}`
      : "新建插件";

  if (!draft) {
    return (
      <>
        <PluginViewHeader title={title} onBack={() => void back()} />
        {editing ? (
          loadError ? (
            <SettingsWarning role="alert">{loadError}</SettingsWarning>
          ) : (
            <SettingsEmptyMessage role="status">正在读取插件…</SettingsEmptyMessage>
          )
        ) : (
          <GroupList title="想做哪一种插件？">
            {AUTHORED_KINDS.filter(
              (entry) =>
                tables || (entry.kind !== "phrase_table" && entry.kind !== "command_table"),
            ).map((entry) => (
              <PluginNavRow
                key={entry.kind}
                title={entry.title}
                description={entry.description}
                rowKey={`create/${entry.kind}`}
                onOpen={() => pick(entry.kind)}
              />
            ))}
          </GroupList>
        )}
      </>
    );
  }

  const addBulk = () => {
    if (draft.kind === "phrase_table") {
      const parsed = parsePhraseLines(bulk);
      setBulkIssue(parsed.issue);
      if (parsed.issue) return;
      update({ phrases: appendRows(draft.phrases, parsed.rows, blankRow.phrase) });
    } else if (draft.kind === "wordbook") {
      const parsed = parseWordLines(bulk);
      setBulkIssue(parsed.issue);
      if (parsed.issue) return;
      update({ words: appendRows(draft.words, parsed.rows, blankRow.word) });
    }
    setBulk("");
  };

  return (
    <>
      <PluginViewHeader title={title} onBack={() => void back()} />
      <GroupList title="基本信息">
        {/* 许可证怎么填收进组名旁的「?」，不在表单中间常驻一段。 */}
        <SettingsGroupNote>
          许可证决定别人能怎样使用这个插件。{DEFAULT_LICENSE}{" "}
          表示放弃全部权利，任何人都能随意使用和改编；想让别人注明出自你，可以写
          CC-BY-4.0。这里只能写 SPDX 许可证代码。
        </SettingsGroupNote>
        <SettingsManagerBlock>
          <SettingsPhraseForm>
            <SettingsInputField
              label="名字"
              ariaLabel="插件名字"
              value={draft.name}
              placeholder={`我的${kindLabel}`}
              onChange={(name) => update({ name })}
            />
            <SettingsInputField
              label="作者（可不填）"
              ariaLabel="作者"
              value={draft.author}
              onChange={(author) => update({ author })}
            />
            <SettingsInputField
              label="版本"
              ariaLabel="版本"
              value={draft.version}
              autoCapitalize="off"
              spellCheck={false}
              onChange={(version) => update({ version })}
            />
            <SettingsInputField
              label="许可证"
              ariaLabel="许可证"
              value={draft.license}
              autoCapitalize="off"
              spellCheck={false}
              onChange={(license) => update({ license })}
            />
          </SettingsPhraseForm>
          <SettingsTextareaField
            label="介绍（可不填）"
            ariaLabel="介绍"
            className={descriptionInput}
            value={draft.description}
            onChange={(description) => update({ description })}
          />
        </SettingsManagerBlock>
      </GroupList>
      {draft.kind === "phrase_table" && (
        <PhraseRows
          rows={draft.phrases}
          shown={shown}
          onShowMore={() => setShown((count) => count + ROW_PAGE)}
          onChange={(phrases) => update({ phrases })}
          onAdd={() => {
            setShown((count) => Math.max(count, draft.phrases.length + 1));
            update({ phrases: [...draft.phrases, emptyPhrase()] });
          }}
        />
      )}
      {draft.kind === "command_table" && (
        <CommandRows
          rows={draft.commands}
          onChange={(commands) => update({ commands })}
          onAdd={() => update({ commands: [...draft.commands, emptyCommand()] })}
        />
      )}
      {draft.kind === "symbol_set" && (
        <SymbolGroups
          groups={draft.groups}
          onChange={(groups) => update({ groups })}
          onAdd={() => update({ groups: [...draft.groups, emptyGroup()] })}
        />
      )}
      {draft.kind === "wordbook" && (
        <WordRows
          rows={draft.words}
          shown={shown}
          onShowMore={() => setShown((count) => count + ROW_PAGE)}
          onChange={(words) => update({ words })}
          onAdd={() => {
            setShown((count) => Math.max(count, draft.words.length + 1));
            update({ words: [...draft.words, emptyWord()] });
          }}
        />
      )}
      {(draft.kind === "phrase_table" || draft.kind === "wordbook") && (
        <GroupList title="批量添加">
          <SettingsManagerBlock>
            <SettingsTextareaField
              label={draft.kind === "phrase_table" ? "每行一条短语" : "每行一个单词"}
              description={
                draft.kind === "phrase_table"
                  ? "编码在前，文字在后，中间用空格隔开，例如 dz 北京市海淀区。"
                  : "从表格复制过来即可：单词、释义两列，或单词、音标、释义三列，列之间用 Tab 隔开。"
              }
              ariaLabel="批量添加的内容"
              className={listInput}
              value={bulk}
              onChange={(value) => {
                setBulk(value);
                setBulkIssue(null);
              }}
            />
            {bulkIssue && <SettingsWarning role="alert">{bulkIssue}</SettingsWarning>}
            <SettingsManagerActions>
              <ActionButton action={addBulk} disabled={!bulk.trim()} label="添加到列表" />
            </SettingsManagerActions>
          </SettingsManagerBlock>
        </GroupList>
      )}
      <GroupList>
        <SettingsManagerBlock>
          <SettingsManagerNote>
            {editing
              ? "保存后替换本机的这一份。从社区装来的插件改过之后就是你自己的副本，原作者那边不受影响。"
              : "保存后装进本机，和导入的插件一样出现在「我的插件」里，也可以在「社区插件」里发布给别人。"}
          </SettingsManagerNote>
          {issue && dirty && <SettingsWarning role="alert">{issue}</SettingsWarning>}
          {saveError && <SettingsWarning role="alert">{saveError}</SettingsWarning>}
          <SettingsManagerActions>
            <ActionButton
              action={() => void save()}
              className="primary"
              ariaBusy={saving}
              disabled={saving || !dirty || issue !== null}
              label={editing ? "保存修改" : "保存并安装"}
            />
          </SettingsManagerActions>
        </SettingsManagerBlock>
      </GroupList>
    </>
  );
}

/** 列表行数超过一次画出的数量时，接着画下一批。 */
function MoreRows({
  total,
  shown,
  onShowMore,
}: {
  total: number;
  shown: number;
  onShowMore: () => void;
}) {
  if (total <= shown) return null;
  return (
    <SettingsManagerActions>
      <ActionButton
        action={onShowMore}
        label={`再显示 ${Math.min(ROW_PAGE, total - shown)} 条（共 ${total} 条）`}
      />
    </SettingsManagerActions>
  );
}

const replaceAt = <T,>(rows: readonly T[], index: number, patch: Partial<T>): T[] =>
  rows.map((row, position) => (position === index ? { ...row, ...patch } : row));
const removeAt = <T,>(rows: readonly T[], index: number): T[] =>
  rows.filter((_, position) => position !== index);

function PhraseRows({
  rows,
  shown,
  onShowMore,
  onChange,
  onAdd,
}: {
  rows: PluginPhrase[];
  shown: number;
  onShowMore: () => void;
  onChange: (rows: PluginPhrase[]) => void;
  onAdd: () => void;
}) {
  return (
    <GroupList title={`短语（${rows.length}）`}>
      <SettingsGroupNote>
        编码是 K 之后输入的小写字母，最多 32 个；同一个编码可以对应几段文字。文字最多{" "}
        {MAX_TEXT_UTF16} 个字符，不能换行。
      </SettingsGroupNote>
      <SettingsGroupNote>{QUICK_PHRASE_CURSOR_NOTE}</SettingsGroupNote>
      <SettingsManagerBlock>
        {rows.slice(0, shown).map((row, index) => (
          <SettingsPhraseForm key={index}>
            <SettingsInputField
              label="编码"
              ariaLabel={`第 ${index + 1} 条短语的编码`}
              value={row.key}
              placeholder="dz"
              autoCapitalize="off"
              spellCheck={false}
              onChange={(key) => onChange(replaceAt(rows, index, { key }))}
            />
            <SettingsInputField
              label="文字"
              ariaLabel={`第 ${index + 1} 条短语的文字`}
              value={row.text}
              onChange={(text) => onChange(replaceAt(rows, index, { text }))}
            />
            <ActionButton
              action={() => onChange(removeAt(rows, index))}
              ariaLabel={`删除第 ${index + 1} 条短语`}
              label="删除"
            />
          </SettingsPhraseForm>
        ))}
        <MoreRows total={rows.length} shown={shown} onShowMore={onShowMore} />
        <SettingsManagerActions>
          <ActionButton action={onAdd} disabled={rows.length >= MAX_PHRASES} label="添加一条" />
        </SettingsManagerActions>
      </SettingsManagerBlock>
    </GroupList>
  );
}

function CommandRows({
  rows,
  onChange,
  onAdd,
}: {
  rows: PluginCommand[];
  onChange: (rows: PluginCommand[]) => void;
  onAdd: () => void;
}) {
  return (
    <GroupList title={`指令（${rows.length}）`}>
      <SettingsGroupNote>
        指令是 / 之后输入的小写字母。要插入的文字里可以写 {"{date}"}（日期）、{"{time}"}（时间）和{" "}
        {"{weekday}"}（星期几），例如「{"{date}"} 张三」；日期和时间也可以带格式，例如{" "}
        {"{date:%m月%d日}"}。
      </SettingsGroupNote>
      <SettingsManagerBlock>
        {rows.map((row, index) => (
          <SettingsPhraseForm key={index}>
            <SettingsInputField
              label="指令"
              ariaLabel={`第 ${index + 1} 条指令的字母`}
              value={row.trigger}
              placeholder="sig"
              autoCapitalize="off"
              spellCheck={false}
              onChange={(trigger) => onChange(replaceAt(rows, index, { trigger }))}
            />
            <SettingsInputField
              label="标题"
              ariaLabel={`第 ${index + 1} 条指令的标题`}
              value={row.title}
              placeholder="签名"
              onChange={(title) => onChange(replaceAt(rows, index, { title }))}
            />
            <SettingsInputField
              label="要插入的文字"
              ariaLabel={`第 ${index + 1} 条指令要插入的文字`}
              value={row.template}
              onChange={(template) => onChange(replaceAt(rows, index, { template }))}
            />
            <ActionButton
              action={() => onChange(removeAt(rows, index))}
              ariaLabel={`删除第 ${index + 1} 条指令`}
              label="删除"
            />
          </SettingsPhraseForm>
        ))}
        <SettingsManagerActions>
          <ActionButton action={onAdd} disabled={rows.length >= MAX_COMMANDS} label="添加一条" />
        </SettingsManagerActions>
      </SettingsManagerBlock>
    </GroupList>
  );
}

function SymbolGroups({
  groups,
  onChange,
  onAdd,
}: {
  groups: DraftGroup[];
  onChange: (groups: DraftGroup[]) => void;
  onAdd: () => void;
}) {
  return (
    <>
      {groups.map((group, index) => (
        <GroupList key={index} title={`第 ${index + 1} 组`}>
          <SettingsManagerBlock>
            <SettingsPhraseForm>
              <label className={settings.field}>
                放在
                <ControlledSelect
                  className={settings.fieldSelect}
                  aria-label={`第 ${index + 1} 组放在哪一页`}
                  value={group.tab}
                  onChange={(tab) =>
                    onChange(replaceAt(groups, index, { tab: tab as DraftGroup["tab"] }))
                  }
                >
                  <option value="symbols">符号</option>
                  <option value="kaomoji">颜文字</option>
                </ControlledSelect>
              </label>
              <SettingsInputField
                label="标题"
                ariaLabel={`第 ${index + 1} 组的标题`}
                value={group.title}
                placeholder="箭头"
                onChange={(title) => onChange(replaceAt(groups, index, { title }))}
              />
              <SettingsInputField
                label="搜索关键词（可不填）"
                ariaLabel={`第 ${index + 1} 组的搜索关键词`}
                value={group.keywords}
                placeholder="jiantou arrow"
                onChange={(keywords) => onChange(replaceAt(groups, index, { keywords }))}
              />
            </SettingsPhraseForm>
            <SettingsTextareaField
              label="符号"
              description="每行一个，可以直接粘贴。"
              ariaLabel={`第 ${index + 1} 组的符号`}
              className={listInput}
              value={group.items}
              onChange={(items) => onChange(replaceAt(groups, index, { items }))}
            />
            <SettingsManagerActions>
              <ActionButton
                action={() => onChange(removeAt(groups, index))}
                ariaLabel={`删除第 ${index + 1} 组`}
                label="删除这一组"
              />
            </SettingsManagerActions>
          </SettingsManagerBlock>
        </GroupList>
      ))}
      <GroupList>
        <SettingsManagerBlock>
          <SettingsManagerNote>
            符号组以插件名为分类排在符号面板的内置符号之后，颜文字组排在颜文字的 All 之后。
          </SettingsManagerNote>
          <SettingsManagerActions>
            <ActionButton action={onAdd} disabled={groups.length >= MAX_GROUPS} label="添加一组" />
          </SettingsManagerActions>
        </SettingsManagerBlock>
      </GroupList>
    </>
  );
}

function WordRows({
  rows,
  shown,
  onShowMore,
  onChange,
  onAdd,
}: {
  rows: DraftWord[];
  shown: number;
  onShowMore: () => void;
  onChange: (rows: DraftWord[]) => void;
  onAdd: () => void;
}) {
  return (
    <GroupList title={`单词（${rows.length}）`}>
      <SettingsGroupNote>
        每个单词要有释义，音标可以不填。单词本的名字最多 64 个字，最多 {MAX_WORDS} 个单词。
      </SettingsGroupNote>
      <SettingsManagerBlock>
        {rows.slice(0, shown).map((row, index) => (
          <SettingsPhraseForm key={index}>
            <SettingsInputField
              label="单词"
              ariaLabel={`第 ${index + 1} 个单词`}
              value={row.word}
              autoCapitalize="off"
              spellCheck={false}
              onChange={(word) => onChange(replaceAt(rows, index, { word }))}
            />
            <SettingsInputField
              label="音标（可不填）"
              ariaLabel={`第 ${index + 1} 个单词的音标`}
              value={row.phonetic}
              autoCapitalize="off"
              spellCheck={false}
              onChange={(phonetic) => onChange(replaceAt(rows, index, { phonetic }))}
            />
            <SettingsInputField
              label="释义"
              ariaLabel={`第 ${index + 1} 个单词的释义`}
              value={row.gloss}
              onChange={(gloss) => onChange(replaceAt(rows, index, { gloss }))}
            />
            <ActionButton
              action={() => onChange(removeAt(rows, index))}
              ariaLabel={`删除第 ${index + 1} 个单词`}
              label="删除"
            />
          </SettingsPhraseForm>
        ))}
        <MoreRows total={rows.length} shown={shown} onShowMore={onShowMore} />
        <SettingsManagerActions>
          <ActionButton action={onAdd} disabled={rows.length >= MAX_WORDS} label="添加一个" />
        </SettingsManagerActions>
      </SettingsManagerBlock>
    </GroupList>
  );
}
