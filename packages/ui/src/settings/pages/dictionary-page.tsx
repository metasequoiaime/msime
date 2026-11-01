import { useEffect, useRef, useState } from "react";
import { SettingsGroupNote } from "../settings-group-note";
import * as settings from "../settings-style";
import { SettingsManagerBlock } from "../settings-manager-block";
import type { LocalDictionaryKind, DictionaryPageFormat } from "../../index";
import {
  localDictionaryKinds,
  resourceDictionaryFromOption,
  resourceDictionaryKinds,
  resourceDictionaryOption,
} from "../../dictionary/dictionary-kinds";
import {
  DICTIONARY_PAGE_SIZE,
  type ResourceDictionaryKind,
} from "../../dictionary/dictionary-file";
import { CANTONESE_NEW_WORD_WEIGHT } from "../../dictionary/dictionary-collections";
import { useSettingsForm } from "../settings-form-context";
import { SubPageEntries } from "./sub-page-entries";
import { GroupList } from "../../core/platform-controls";
import { DictionaryUpdates } from "../dictionary-updates";
import { DictionaryManifestCard } from "../dictionary-manifest-card";
import { PersonalDictionaryImportCard } from "../personal-dictionary-import-card";
import { DictionaryEntries } from "../dictionary-entries";
import { DictionaryFailuresNotice } from "../dictionary-failures-notice";
import { DictionaryPagination } from "../dictionary-pagination";
import { TextInputRow } from "../text-input-row";
import { SelectRow } from "../select-row";
import { dictionaryFormatOptions } from "../../dictionary/dictionary-format-options";
import { OpenPanelRow } from "../open-panel-row";
import { ActionRow } from "../action-row";
import { ActionButton } from "../action-button";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { ErrorAlert } from "../../core/error-alert";
import { SettingsInputDescription } from "../settings-input-description";
import { SettingsManagerActions } from "../settings-manager-actions";
import { SettingsEmptyMessage } from "../settings-empty-message";
import { HarmonyPhoneDictionary } from "../harmony-phone-dictionary";
import { useSettingsTabs } from "../use-settings-tabs";
import { cloudDictionaryTab, dictionaryTabs } from "../dictionary-tabs";
import { CloudDictionaryTab } from "../cloud-dictionary-tab";
import { ResourceDictionaryLookup } from "../resource-dictionary-lookup";
import { DictionarySyncSection } from "../dictionary-sync-section";

export { dictionaryKindKeyHint } from "../../dictionary/dictionary-messages";

/** 按扩展名推断这次导入的格式：`.yaml`、`.yml` 是 Rime，`.scel` 是搜狗细胞词库，其余用页面上选的格式。改了扩展名的细胞词库由导入时检查文件头兜住。 */
function fileFormat(fileName: string, chosen: DictionaryPageFormat): DictionaryPageFormat {
  const name = fileName.toLowerCase();
  if (name.endsWith(".yaml") || name.endsWith(".yml")) return "rime";
  if (name.endsWith(".scel")) return "scel";
  return chosen;
}

/** 词库查询、个人词库管理与云词库共用顶部标签。 */
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
    learnedDataError,
    learnedDataNotice,
    clearPhraseFeedback,
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
    harmonyPlatform,
    mobilePlatform,
    openCommunityGallery,
    openPanel,
    openExternalUrl,
    dictionaryTab,
    setDictionaryTab,
  } = useSettingsForm();
  const [resourceKind, setResourceKind] = useState<ResourceDictionaryKind | null>(null);
  const contribution = client.openExternalUrl && (
    <GroupList title="完善公共词库">
      <ActionRow
        title="提交缺失词条"
        description="前往官网填写词语和读音，审核通过后随词库更新提供给所有人。个人学习的词不会自动公开。"
        action={() => void openExternalUrl("https://msime.app/words/")}
        label="前往投稿"
      />
    </GroupList>
  );
  // 「清除全部学习数据」在「管理词库」页末，桌面把它的结果画在「学习数据」组里、按钮下面：画在页顶的话，用户滚到页末点完按钮什么也看不到。
  const learnedDataFeedback =
    learnedDataError || learnedDataNotice ? (
      <>
        {learnedDataError && <ErrorAlert>{learnedDataError}</ErrorAlert>}
        {learnedDataNotice && (
          <SettingsEmptyMessage compact role="status">
            {learnedDataNotice}
          </SettingsEmptyMessage>
        )}
      </>
    ) : null;
  // 等待同步、快照错误和失败记录是词库本身的状态，导入后由重新读取的列表带回来；桌面画在标签下、面板之上，在哪个标签上都看得到，重试和移除按钮也只有这一份。操作结果（`phraseError`、`phraseNotice`）同样画在这里，但切换标签时清掉。
  const feedback = (
    <>
      {dictionaryPendingCount > 0 && (
        <SettingsInputDescription role="status">
          {dictionaryPendingCount} 项等待键盘同步。打开水杉键盘后会在空闲时逐条生效。
        </SettingsInputDescription>
      )}
      {dictionarySnapshotError && <ErrorAlert>{dictionarySnapshotError}</ErrorAlert>}
      <DictionaryFailuresNotice
        failures={dictionaryFailures}
        busy={phraseBusy}
        canRetry={Boolean(client.dictionary?.retry)}
        canDismiss={Boolean(client.dictionary?.dismissFailure)}
        onRetry={(requestId) => void retryDictionaryFailure(requestId)}
        onDismiss={(requestId) => void dismissDictionaryFailure(requestId)}
      />
      {phraseError && <ErrorAlert>{phraseError}</ErrorAlert>}
      {phraseNotice && (
        <SettingsEmptyMessage compact role="status">
          {phraseNotice}
        </SettingsEmptyMessage>
      )}
      {/* 手机直列，清除学习数据的结果仍和其他提示画在一起，位置不变。 */}
      {mobilePlatform && learnedDataFeedback}
    </>
  );
  const tabs =
    client.cloudDictionary && !mobilePlatform
      ? ([...dictionaryTabs, cloudDictionaryTab] as const)
      : dictionaryTabs;
  const shownTabs = tabs.filter((tab) => tab.id !== "lookup" || Boolean(client.dictionary));
  const { tabList, panel, selected } = useSettingsTabs(
    shownTabs,
    "词库设置分类",
    // 没有本地词库的桌面宿主只剩「管理词库」一项，没有可切换的东西，像手机一样不画标签栏、直接展示。不在列表里的两个面板这时内容为空（「查找词库」要本地词库，「云端词库」有了才会进列表），不会多出东西。
    !mobilePlatform && shownTabs.length > 1,
    dictionaryTab,
    setDictionaryTab,
    settings.dictionaryTabs,
  );
  // 桌面切换标签时清掉上一个标签留下的操作结果，例如「查找词库」里的查询错误不该留在「管理词库」的导入导出上面；外壳从别处切换标签（账号页打开「云端词库」）也一样。手机没有标签，不受影响。
  const shownTab = useRef(selected);
  useEffect(() => {
    if (shownTab.current === selected) return;
    shownTab.current = selected;
    if (!mobilePlatform) clearPhraseFeedback();
  });
  // HarmonyOS 手机按 Android 的词库页布局：按词库列出，点进去编辑，而不是这张按种类查词条的桌面表单。它在打开时读词库和社区列表，所以只在本页可见时挂载。
  if (harmonyPlatform && mobilePlatform) {
    return (
      <SettingsPageFieldset disabled={busy} hidden={page !== "dictionary"} ariaLabel="词库">
        {page === "dictionary" && (
          <>
            {client.dictionary?.updates && (
              <DictionaryUpdates request={client.dictionary.updates} />
            )}
            <HarmonyPhoneDictionary />
            <DictionarySyncSection />
            {contribution}
          </>
        )}
      </SettingsPageFieldset>
    );
  }
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "dictionary"} ariaLabel="词库">
      <div className={settings.dictionaryNav}>
        <div className={settings.dictionaryNavCopy}>
          <span className={settings.dictionaryNavEyebrow}>词库工具</span>
          <span className={settings.dictionaryNavDescription}>管理本机词条与云端词库</span>
        </div>
        {tabList}
      </div>
      {!mobilePlatform && feedback}
      {contribution}
      {/* 先是词库本身（查、看、加），再是成批的导入导出和云词库，然后是只读的词库信息和别的入口，清除学习数据这种危险操作放在页末。 */}
      {panel(
        "lookup",
        client.dictionary && (
          <GroupList title="本地词库管理">
            <SettingsGroupNote>
              默认显示所选词库的全部词条，输入文字或编码后实时筛选。个人词条可以编辑，基础词库只读。
            </SettingsGroupNote>
            <SelectRow
              title="词库"
              aria-label="本地词库类型"
              className={settings.dictionaryKindSelect}
              value={resourceKind ? resourceDictionaryOption(resourceKind) : dictionaryKind}
              onChange={(event) => {
                const resource = resourceDictionaryFromOption(event.target.value);
                setPhraseForm(null);
                setPhraseSearch("");
                setPhrases([]);
                if (resource) {
                  setResourceKind(resource);
                  return;
                }
                const kind = event.target.value as LocalDictionaryKind;
                setResourceKind(null);
                if (kind === dictionaryKind && !phraseSearch) void loadPhrases(kind, 0);
                setDictionaryKind(kind);
                if (
                  kind !== "pinyin" &&
                  (dictionaryFormat === "hans" || dictionaryFormat === "scel")
                )
                  setDictionaryFormat("standard");
                setPhrases([]);
              }}
            >
              {localDictionaryKinds.map(([kind, label]) => (
                <option key={kind} value={kind}>
                  {label}
                </option>
              ))}
              {client.dictionary.browse &&
                resourceDictionaryKinds.map(([kind, label]) => (
                  <option key={`resource:${kind}`} value={resourceDictionaryOption(kind)}>
                    {label}
                  </option>
                ))}
            </SelectRow>
            {resourceKind ? (
              <ResourceDictionaryLookup
                key={resourceKind}
                dictionary={client.dictionary}
                kind={resourceKind}
              />
            ) : (
              <>
                <TextInputRow
                  title="词条或编码"
                  label="词条或编码"
                  className={settings.fieldInput}
                  type="search"
                  value={phraseSearch}
                  placeholder="例如：你好 或 nihao"
                  onChange={setPhraseSearch}
                />
                <SettingsManagerBlock>
                  {mobilePlatform && feedback}
                  <DictionaryEntries
                    kind={dictionaryKind}
                    entries={phrases}
                    form={phraseForm}
                    busy={phraseBusy}
                    emptyMessage={phrasePage.status}
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
                  <div className={settings.dictionaryFooter}>
                    <ActionButton
                      action={() =>
                        setPhraseForm({
                          key: "",
                          value: "",
                          weight: dictionaryKind === "cantonese" ? CANTONESE_NEW_WORD_WEIGHT : 10,
                          previous: null,
                        })
                      }
                      disabled={phraseBusy}
                      label="新增词条"
                    />
                    <DictionaryPagination
                      busy={phraseBusy}
                      offset={phrasePage.offset}
                      hasMore={phrasePage.hasMore}
                      status={phrasePage.status}
                      pageSize={DICTIONARY_PAGE_SIZE}
                      onPageChange={turnPhrasePage}
                    />
                  </div>
                </SettingsManagerBlock>
              </>
            )}
          </GroupList>
        ),
      )}
      {panel(
        "cloud",
        client.cloudDictionary && (
          <div className={settings.cloudDictionaryEmbed}>
            <CloudDictionaryTab
              client={client.cloudDictionary}
              onClose={() => setDictionaryTab("lookup")}
            />
          </div>
        ),
      )}
      {panel(
        "manage",
        <>
          {client.dictionary && (
            <GroupList title="导入与导出">
              <SettingsGroupNote>
                {client.dictionary.scelText
                  ? "导入和导出个人新增词条；导入支持标准、Windows TSV、Rime、纯汉字自动注音，拼音词库还能导入搜狗细胞词库（.scel）。"
                  : "导入和导出个人新增词条；导入支持标准、Windows TSV、Rime 和纯汉字自动注音。"}
              </SettingsGroupNote>
              <SelectRow
                title="个人词库"
                aria-label="管理词库类型"
                value={dictionaryKind}
                disabled={phraseBusy}
                onChange={(event) => {
                  const kind = event.target.value as LocalDictionaryKind;
                  setDictionaryKind(kind);
                  if (
                    kind !== "pinyin" &&
                    (dictionaryFormat === "hans" || dictionaryFormat === "scel")
                  )
                    setDictionaryFormat("standard");
                }}
              >
                {localDictionaryKinds.map(([kind, label]) => (
                  <option key={kind} value={kind}>
                    {label}
                  </option>
                ))}
              </SelectRow>
              <SelectRow
                title="文件格式"
                aria-label="本地词库文件格式"
                value={dictionaryFormat}
                disabled={phraseBusy}
                onChange={(event) =>
                  setDictionaryFormat(event.target.value as DictionaryPageFormat)
                }
              >
                {dictionaryFormatOptions({
                  pinyin: dictionaryKind === "pinyin",
                  rime: true,
                  scel: dictionaryKind === "pinyin" && Boolean(client.dictionary.scelText),
                })}
              </SelectRow>
              <SettingsManagerBlock>
                <SettingsManagerActions>
                  <label className="secondary">
                    导入
                    <input
                      hidden
                      type="file"
                      accept={
                        client.dictionary.scelText
                          ? ".txt,.tsv,.yaml,.yml,.scel,text/plain"
                          : ".txt,.tsv,.yaml,.yml,text/plain"
                      }
                      disabled={phraseBusy}
                      onChange={(event) => {
                        const file = event.target.files?.[0];
                        if (file) {
                          // 按扩展名换的格式直接交给这次导入：刚设的状态要到下一次渲染才生效。
                          const format = fileFormat(file.name, dictionaryFormat);
                          if (
                            format !== dictionaryFormat &&
                            (format !== "scel" || dictionaryKind === "pinyin")
                          )
                            setDictionaryFormat(format);
                          void importPhrases(file, format);
                        }
                        event.currentTarget.value = "";
                      }}
                    />
                  </label>
                  <ActionButton
                    action={() => void exportPhrases()}
                    disabled={
                      phraseBusy || dictionaryFormat === "hans" || dictionaryFormat === "scel"
                    }
                    label="导出当前类型"
                  />
                  <ActionButton
                    action={() => void exportAllPhrases()}
                    disabled={phraseBusy}
                    label="导出全部"
                  />
                </SettingsManagerActions>
              </SettingsManagerBlock>
              {client.dictionary.importPersonal && (
                <PersonalDictionaryImportCard
                  embedded
                  dictionary={client.dictionary}
                  platform={client.host?.platform}
                />
              )}
            </GroupList>
          )}
          {/* 「我的词条」经网盘同步，紧跟着个人词条的导入导出；宿主不提供时整组不显示。 */}
          <DictionarySyncSection />
          {client.openCloudDictionary &&
            client.host?.platform !== "macos" &&
            (!client.cloudDictionary || mobilePlatform) && (
              <GroupList title="云词库">
                <OpenPanelRow
                  title="云词库"
                  description="管理同步到账号的词条和备份"
                  action={() => openPanel(client.openCloudDictionary)}
                  label="打开云词库"
                />
              </GroupList>
            )}
          {/* 触屏宿主的社区词库也从这里进入：「浏览更多词库」切到底部「社区」标签的「词库」分类。 */}
          {mobilePlatform && client.communityResources && (
            <GroupList title="社区词库">
              <ActionRow
                title="社区词库"
                description="看看别人分享的词库，可以添加到本机"
                action={() =>
                  openCommunityGallery({ category: "dictionary", scope: "", initialMine: false })
                }
                label="浏览更多词库"
              />
            </GroupList>
          )}
          {client.dictionary?.updates && <DictionaryUpdates request={client.dictionary.updates} />}
          {client.dictionaryManifest && <DictionaryManifestCard read={client.dictionaryManifest} />}
          <SubPageEntries
            title="更多"
            pages={[{ id: "vocabulary", description: "用输入过的英文单词复习词汇" }]}
          />
          {/* 宿主提供 `resetLearnedData` 就显示（宿主只在真正能清除的桌面平台上提供）；它是危险操作，放在页末。 */}
          {client.resetLearnedData && (
            <GroupList title="学习数据">
              <ActionRow
                title="清除候选词频、用户词库和拼音学习记录"
                description="自己新增和修改的词条也会删除；输入方案与其他设置不会改变。"
                action={resetLearnedData}
                className="secondary danger-button"
                disabled={phraseBusy}
                label="清除全部学习数据"
              />
              {!mobilePlatform && learnedDataFeedback && (
                <SettingsManagerBlock>{learnedDataFeedback}</SettingsManagerBlock>
              )}
            </GroupList>
          )}
        </>,
      )}
    </SettingsPageFieldset>
  );
}
