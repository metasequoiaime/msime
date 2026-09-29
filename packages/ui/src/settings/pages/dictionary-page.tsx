import * as settings from "../settings-style";
import type { LocalDictionaryKind, LocalDictionaryFormat } from "../../index";
import { localDictionaryKinds } from "../settings-options";
import { DICTIONARY_PAGE_SIZE } from "../../dictionary/dictionary-file";
import { useSettingsForm } from "../settings-form-context";
import { SubPageEntries } from "./sub-page-entries";
import { GroupList, Row, Select } from "../../core/platform-controls";
import { DictionaryManifestCard } from "../dictionary-manifest-card";
import { PersonalDictionaryImportCard } from "../personal-dictionary-import-card";

/** The encoding rules shown beside the Apple personal-dictionary editor. */
export function dictionaryKindKeyHint(kind: LocalDictionaryKind): string {
  switch (kind) {
    case "wubi":
      return "1–4 个字母";
    case "quick_phrase":
      return "1–32 个字母";
    case "english":
      return "1–64 个字母";
    case "pinyin":
      return "完整音节，用 ' 分隔，如 ni'hao";
  }
}

/** The 词库 page of the settings form. */
export function DictionarySettingsPage() {
  const {
    client,
    macosPlatform,
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
        <SubPageEntries
          title="学习"
          pages={[{ id: "vocabulary", description: "用输入过的英文单词复习词汇" }]}
        />
        {client.dictionaryManifest && <DictionaryManifestCard read={client.dictionaryManifest} />}
        {client.dictionary?.importPersonal && (
          <PersonalDictionaryImportCard
            dictionary={client.dictionary}
            platform={client.host?.platform}
          />
        )}
        {client.dictionary && (
          <GroupList title="本地词库管理">
            <p className={settings.groupNote}>
              查询、新增、编辑、导入、导出和删除 Engine 用户词库。导入支持标准、Windows TSV、Rime
              和纯汉字自动注音。标有「内置」的是随输入法附带的词条，只能调整权重或删除。
            </p>
            <div className={settings.managerBlock}>
              <div className={settings.managerActions}>
                <button
                  type="button"
                  className="secondary"
                  disabled={phraseBusy}
                  onClick={() => void loadPhrases(dictionaryKind, 0)}
                >
                  查询
                </button>
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
              </div>
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
              {dictionaryFailures.length > 0 && (
                <div className={settings.failures} role="alert">
                  <p>有 {dictionaryFailures.length} 项词库请求同步失败，可以重试或移除失败记录。</p>
                  <ul>
                    {dictionaryFailures.map((failure) => (
                      <li key={failure.request_id}>
                        <span>
                          <strong>{failure.label}</strong>
                          <small>{failure.error}</small>
                        </span>
                        <span>
                          <button
                            type="button"
                            className="secondary"
                            disabled={phraseBusy || !client.dictionary?.retry}
                            onClick={() => void retryDictionaryFailure(failure.request_id)}
                          >
                            重试
                          </button>{" "}
                          <button
                            type="button"
                            className="secondary"
                            disabled={phraseBusy || !client.dictionary?.dismissFailure}
                            onClick={() => void dismissDictionaryFailure(failure.request_id)}
                          >
                            移除记录
                          </button>
                        </span>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
            <Row title="词库">
              <Select
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
              </Select>
            </Row>
            <Row title="文件格式">
              <Select
                aria-label="本地词库文件格式"
                value={dictionaryFormat}
                disabled={phraseBusy}
                onChange={(event) =>
                  setDictionaryFormat(event.target.value as LocalDictionaryFormat)
                }
              >
                <option value="standard">词在前（标准 TSV）</option>
                <option value="windows">编码在前（Windows TSV）</option>
                <option value="rime">Rime userdb / dict.yaml</option>
                {dictionaryKind === "pinyin" && (
                  <option value="hans">汉字自动注音（仅导入）</option>
                )}
              </Select>
            </Row>
            <Row title="编码前缀">
              <input
                aria-label="编码前缀"
                className={settings.fieldInput}
                value={phraseSearch}
                placeholder="留空查看全部"
                onChange={(event) => setPhraseSearch(event.target.value)}
              />
            </Row>
            <div className={settings.managerBlock}>
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
              {phraseForm && (
                <div className={settings.phraseForm}>
                  <label>
                    编码{" "}
                    <input
                      value={phraseForm.key}
                      readOnly={phraseForm.previous?.source === "bundled"}
                      onChange={(event) =>
                        setPhraseForm({ ...phraseForm, key: event.target.value })
                      }
                    />
                    <small className={settings.keyHint}>
                      {dictionaryKindKeyHint(dictionaryKind)}
                    </small>
                  </label>
                  <label>
                    {dictionaryKind === "quick_phrase" ? "短语" : "词条"}{" "}
                    <input
                      value={phraseForm.value}
                      readOnly={phraseForm.previous?.source === "bundled"}
                      onChange={(event) =>
                        setPhraseForm({ ...phraseForm, value: event.target.value })
                      }
                    />
                  </label>
                  <label>
                    权重{" "}
                    <input
                      type="number"
                      value={phraseForm.weight}
                      onChange={(event) =>
                        setPhraseForm({
                          ...phraseForm,
                          weight: Number(event.target.value),
                        })
                      }
                    />
                  </label>
                  <button type="button" disabled={phraseBusy} onClick={() => void savePhrase()}>
                    保存
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    disabled={phraseBusy}
                    onClick={() => setPhraseForm(null)}
                  >
                    取消
                  </button>
                </div>
              )}
              {phrases.length === 0 ? (
                <p className={settings.empty}>
                  点击查询后查看
                  {localDictionaryKinds.find(([kind]) => kind === dictionaryKind)?.[1] ?? "词库"}
                  词条
                </p>
              ) : (
                <ul ref={phraseListRef} className={settings.phraseList} aria-label="词库查询结果">
                  {phrases.map((entry, index) => (
                    <li key={`${entry.key}-${entry.value}-${index}`}>
                      <span>
                        <code>{entry.key}</code>　{entry.value}　<small>{entry.weight}</small>
                        {entry.source === "bundled" && (
                          <>
                            {" "}
                            <small className={settings.bundledBadge}>内置</small>
                          </>
                        )}
                      </span>
                      <span>
                        <button
                          type="button"
                          className="secondary"
                          disabled={phraseBusy}
                          onClick={() =>
                            setPhraseForm({
                              key: entry.key,
                              value: entry.value,
                              weight: entry.weight,
                              previous: entry,
                            })
                          }
                        >
                          {entry.source === "bundled" ? "调权重" : "编辑"}
                        </button>{" "}
                        <button
                          type="button"
                          className="secondary"
                          disabled={phraseBusy}
                          onClick={() => void removePhrase(entry)}
                        >
                          删除
                        </button>
                      </span>
                    </li>
                  ))}
                </ul>
              )}
              <div className="flex items-center justify-center gap-4 text-xs text-secondary">
                <button
                  type="button"
                  className="secondary"
                  disabled={phraseBusy || phrasePage.offset === 0}
                  onClick={() =>
                    turnPhrasePage(Math.max(0, phrasePage.offset - DICTIONARY_PAGE_SIZE))
                  }
                >
                  上一页
                </button>
                <span aria-live="polite">{phrasePage.status}</span>
                <button
                  type="button"
                  className="secondary"
                  disabled={phraseBusy || !phrasePage.hasMore}
                  onClick={() => turnPhrasePage(phrasePage.offset + DICTIONARY_PAGE_SIZE)}
                >
                  下一页
                </button>
              </div>
            </div>
          </GroupList>
        )}
        {macosPlatform && client.resetLearnedData && (
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
