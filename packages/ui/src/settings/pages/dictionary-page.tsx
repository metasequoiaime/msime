import * as settings from "../settings-style";
import type { LocalDictionaryKind, LocalDictionaryFormat } from "../../index";
import { localDictionaryKinds } from "../../dictionary/dictionary-kinds";
import { DICTIONARY_PAGE_SIZE } from "../../dictionary/dictionary-file";
import { useSettingsForm } from "../settings-form-context";
import { SubPageEntries } from "./sub-page-entries";
import { GroupList, Row } from "../../core/platform-controls";
import { DictionaryManifestCard } from "../dictionary-manifest-card";
import { PersonalDictionaryImportCard } from "../personal-dictionary-import-card";
import { DictionaryEntries } from "../dictionary-entries";
import { DictionaryFailuresNotice } from "../dictionary-failures-notice";
import { DictionaryPagination } from "../dictionary-pagination";
import { TextInputRow } from "../text-input-row";
import { SelectRow } from "../select-row";
import { DictionaryFormatOptions } from "../../dictionary/dictionary-format-options";

export { dictionaryKindKeyHint } from "../../dictionary/dictionary-messages";

/** The 词库 page of the settings form. */
export function DictionarySettingsPage() {
  const {
    client,
    busy,
    page,
    setPhrases,
    phrases,
    phrasePage,
    phraseBusy,
    phraseError,
    dictionaryPendingCount,
    dictionaryFailures,
    dictionarySnapshotError,
    phraseNotice,
    phraseSearch,
    setPhraseSearch,
    setPhraseForm,
    phraseForm,
    dictionaryKind,
    setDictionaryKind,
    dictionaryFormat,
    setDictionaryFormat,
    phraseListRef,
    loadPhrases,
    turnPhrasePage,
    removePhrase,
    savePhrase,
    importPhrases,
    retryDictionaryFailure,
    dismissDictionaryFailure,
    exportPhrases,
    exportAllPhrases,
    resetLearnedData,
  } = useSettingsForm();
  return (
    <fieldset disabled={busy} hidden={page !== "dictionary"} aria-label="词库">
      <div className={settings.groups}>
        {/* 先是词库本身（查、看、加），再是成批的导入导出，然后是只读的词库信息和别的入口，清除学习数据这种危险操作放在页末。 */}
        {client.dictionary && (
          <GroupList title="本地词库管理">
            <p className={settings.groupNote}>
              查询、新增、编辑和删除用户词库。标有「内置」的是随输入法附带的词条，只能调整权重或删除。
            </p>
            <SelectRow
              title="词库"
              aria-label="本地词库类型"
              value={dictionaryKind}
              disabled={phraseBusy}
              onChange={(event) => {
                const kind = event.target.value as LocalDictionaryKind;
                setDictionaryKind(kind);
                if (kind !== "pinyin" && dictionaryFormat === "hans")
                  setDictionaryFormat("standard");
                setPhrases([]);
                void loadPhrases(kind);
              }}
            >
              {localDictionaryKinds.map(([kind, label]) => (
                <option key={kind} value={kind}>
                  {label}
                </option>
              ))}
            </SelectRow>
            <TextInputRow
              title="编码前缀"
              label="编码前缀"
              className={settings.fieldInput}
              value={phraseSearch}
              placeholder="留空查看全部"
              onChange={setPhraseSearch}
            >
              <button
                type="button"
                className="secondary"
                disabled={phraseBusy}
                onClick={() => void loadPhrases(dictionaryKind, 0)}
              >
                查询
              </button>
            </TextInputRow>
            <div className={settings.managerBlock}>
              {dictionaryPendingCount > 0 && (
                <p className="input-setting-description" role="status">
                  {dictionaryPendingCount} 项等待键盘同步。打开水杉键盘后会在空闲时逐条生效。
                </p>
              )}
              {dictionarySnapshotError && (
                <p role="alert" className="error">
                  {dictionarySnapshotError}
                </p>
              )}
              <DictionaryFailuresNotice
                failures={dictionaryFailures}
                busy={phraseBusy}
                canRetry={Boolean(client.dictionary?.retry)}
                canDismiss={Boolean(client.dictionary?.dismissFailure)}
                onRetry={(requestId) => void retryDictionaryFailure(requestId)}
                onDismiss={(requestId) => void dismissDictionaryFailure(requestId)}
              />
              {phraseError && (
                <p role="alert" className="error">
                  {phraseError}
                </p>
              )}
              {phraseNotice && (
                <p role="status" className={settings.empty}>
                  {phraseNotice}
                </p>
              )}
              <DictionaryEntries
                kind={dictionaryKind}
                entries={phrases}
                form={phraseForm}
                busy={phraseBusy}
                listRef={phraseListRef}
                onFormChange={setPhraseForm}
                onSave={() => void savePhrase()}
                onCancel={() => setPhraseForm(null)}
                onEdit={(entry) =>
                  setPhraseForm({
                    key: entry.key,
                    value: entry.value,
                    weight: entry.weight,
                    previous: entry,
                  })
                }
                onRemove={(entry) => void removePhrase(entry)}
              />
              <DictionaryPagination
                busy={phraseBusy}
                offset={phrasePage.offset}
                hasMore={phrasePage.hasMore}
                status={phrasePage.status}
                pageSize={DICTIONARY_PAGE_SIZE}
                onPageChange={turnPhrasePage}
              />
              <div className={settings.managerActions}>
                <button
                  type="button"
                  className="secondary"
                  disabled={phraseBusy}
                  onClick={() =>
                    setPhraseForm({
                      key: "",
                      value: "",
                      weight: 10,
                      previous: null,
                    })
                  }
                >
                  新增词条
                </button>
              </div>
            </div>
          </GroupList>
        )}
        {client.dictionary && (
          <GroupList title="导入与导出">
            <p className={settings.groupNote}>
              导入和导出的是上面所选类型的词库；导入支持标准、Windows TSV、Rime 和纯汉字自动注音。
            </p>
            <SelectRow
              title="文件格式"
              aria-label="本地词库文件格式"
              value={dictionaryFormat}
              disabled={phraseBusy}
              onChange={(event) => setDictionaryFormat(event.target.value as LocalDictionaryFormat)}
            >
              <DictionaryFormatOptions pinyin={dictionaryKind === "pinyin"} rime />
            </SelectRow>
            <div className={settings.managerBlock}>
              <div className={settings.managerActions}>
                <label className="secondary">
                  导入
                  <input
                    hidden
                    type="file"
                    accept=".txt,.tsv,.yaml,.yml,text/plain"
                    disabled={phraseBusy}
                    onChange={(event) => {
                      const file = event.target.files?.[0];
                      if (file) {
                        const name = file.name.toLowerCase();
                        if (name.endsWith(".yaml") || name.endsWith(".yml"))
                          setDictionaryFormat("rime");
                        void importPhrases(file);
                      }
                      event.currentTarget.value = "";
                    }}
                  />
                </label>
                <button
                  type="button"
                  className="secondary"
                  disabled={phraseBusy || dictionaryFormat === "hans"}
                  onClick={() => void exportPhrases()}
                >
                  导出当前类型
                </button>
                <button
                  type="button"
                  className="secondary"
                  disabled={phraseBusy}
                  onClick={() => void exportAllPhrases()}
                >
                  导出全部
                </button>
              </div>
            </div>
            {client.dictionary.importPersonal && (
              <PersonalDictionaryImportCard
                embedded
                dictionary={client.dictionary}
                platform={client.host?.platform}
              />
            )}
          </GroupList>
        )}
        {client.dictionaryManifest && <DictionaryManifestCard read={client.dictionaryManifest} />}
        <SubPageEntries
          title="更多"
          pages={[{ id: "vocabulary", description: "用输入过的英文单词复习词汇" }]}
        />
        {/* 清除学习数据只要宿主提供就显示，不再只限 macOS；它是危险操作，放在页末。 */}
        {client.resetLearnedData && (
          <GroupList title="学习数据">
            <Row
              title="清除候选词频、用户词典和拼音学习记录"
              description="输入方案与其他设置不会改变。"
            >
              <button
                type="button"
                className="secondary danger-button"
                disabled={phraseBusy}
                onClick={() => void resetLearnedData()}
              >
                清除全部学习数据
              </button>
            </Row>
          </GroupList>
        )}
      </div>
    </fieldset>
  );
}
