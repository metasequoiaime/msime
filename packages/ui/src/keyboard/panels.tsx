import { useKeyboardPanel } from "./use-keyboard-panel";
import { useEmojiPanel, type EmojiPage } from "./use-emoji-panel";
import { useCloudDictionaryFilesPanel } from "./use-cloud-dictionary-files-panel";
import { useCloudCandidatesPanel } from "./use-cloud-candidates-panel";
import { useCloudDictionaryCatalogPanel } from "./use-cloud-dictionary-catalog-panel";
import { useCloudDictionaryApplyPanel } from "./use-cloud-dictionary-apply-panel";
import { CloudDictionaryApplyContent } from "./cloud-dictionary-apply-content";
import { useCloudDictionaryPanel } from "./use-cloud-dictionary-panel";
import { useCloudClipboardPanel } from "./use-cloud-clipboard-panel";
import { useVoicePanel } from "./use-voice-panel";
import { CloudClipboardComposer } from "./cloud-clipboard-composer";
import { CloudClipboardHistory } from "./cloud-clipboard-history";
import { VoiceResults } from "./voice-results";
import type {
  PanelClient,
  VoicePanelClient,
  CloudClipboardPanelClient,
  CloudRankingMode,
  CloudDictionaryFileFormat,
  CloudDictionaryPanelClient,
  ClipboardCaptureStatus,
  EmojiPanelClient,
} from "./panel-client";
import { FluentIcon } from "../core/fluent-icons";
import { ControlledCheckbox } from "../core/controlled-checkbox";
import { clamp } from "../core/number";
import { touchKeyboardSpacing } from "../settings/touch-keyboard-spacing";
import { DictionaryFormatOptions } from "../dictionary/dictionary-format-options";
import { useHandwritingPanel } from "./use-handwriting-panel";
import type { CSSProperties } from "react";
import type { GlobalTheme } from "../theme/global-theme";
import * as cloud from "./cloud-panel-style";
import * as surface from "./panel-surface-style";
import { VoiceLanguageOptions } from "../voice/voice-language-options";
import { keyboardKeyWeight } from "./keyboard-input";
import { KeyboardKeyButton } from "./keyboard-key-button";
import { keyboardSkinStyles } from "./keyboard-skin-styles";
import type { NineKeyShuangpinKeyFace } from "../settings/nine-key-shuangpin-profile";
import { CloudDictionaryEntryForm } from "./cloud-dictionary-entry-form";
import { CloudDictionaryEntryCard } from "./cloud-dictionary-entry-card";
import { CloudCandidateResults, CloudFixedPositions } from "./cloud-candidate-results";
import { CloudPanelHeader } from "./cloud-panel-header";
import { CloudDictionaryPagination } from "./cloud-dictionary-pagination";
import { NativePanelHeader } from "./native-panel-header";
import { PanelActivationButtons } from "./panel-activation-buttons";
import { CloudDictionaryKindTabs } from "./cloud-dictionary-kind-tabs";
import { CloudDictionaryKindSelect } from "./cloud-dictionary-kind-select";
import { CloudDictionarySelectField } from "./cloud-dictionary-select-field";
import { CloudDictionaryQueryToolbar } from "./cloud-dictionary-query-toolbar";
import { CloudPinyinSchemeOptions, CloudShuangpinProfileOptions } from "./cloud-scheme-options";
import { symbolCategoryTitle } from "./emoji-panel-helpers";
import type { EmojiCatalogItem } from "../emoji/emoji-catalog";
import { panelField } from "./emoji-panel-style";
import { EmojiClipboardEntries } from "./emoji-clipboard-entries";
import { EmojiItemButton } from "./emoji-item-button";
import { HandwritingResults } from "./handwriting-results";
import { HandwritingInkEditor } from "./handwriting-ink-editor";
import { HandwritingModelNotice } from "./handwriting-model-notice";

export { emojiDisplayName } from "./emoji-panel-helpers";
import {
  CloudClipboardDeviceList,
  CloudClipboardPendingNotice,
  CloudClipboardWaitingDevices,
} from "./cloud-clipboard-devices";
import type { TouchKeyboardSkinDesign } from "./touch-keyboard-skin-design";

export type {
  KeyboardInputRequest,
  InkPoint,
  HandwritingRecognitionRequest,
  HandwritingRecognitionResult,
  PanelClient,
  VoicePanelClient,
  CloudClipboardAction,
  CloudClipboardItem,
  CloudClipboardResponse,
  CloudClipboardPanelClient,
  CloudDictionaryKind,
  CloudCandidateKind,
  CloudRankingMode,
  CloudDictionaryFileFormat,
  CloudDictionaryEntry,
  CloudDictionaryAction,
  CloudDictionaryCatalogEntry,
  CloudCandidate,
  CloudFixedPosition,
  CloudDictionarySnapshotMetadata,
  CloudDictionarySnapshotRequest,
  CloudDictionaryResponse,
  CloudDictionaryPanelClient,
  EmojiPanelClient,
  ClipboardCaptureStatus,
} from "./panel-client";
export type { InkStroke } from "./panel-client";

export function KeyboardPanel({
  client,
  platform,
  theme = "dark",
  layout = "twenty_six_key",
  nineKeyShuangpinKeys,
  keySpacingTenths,
  rowSpacingTenths,
  voiceShortcut = false,
  skin = "system",
  customDesign,
}: {
  client: PanelClient;
  platform?: string;
  theme?: "dark" | "light";
  layout?: "twenty_six_key" | "nine_key" | "nine_key_shuangpin";
  nineKeyShuangpinKeys?: readonly NineKeyShuangpinKeyFace[];
  keySpacingTenths?: number;
  rowSpacingTenths?: number;
  voiceShortcut?: boolean;
  skin?: GlobalTheme;
  customDesign?: TouchKeyboardSkinDesign;
}) {
  const {
    activeLayout,
    rows,
    renderedModifiers,
    notice,
    openingVoice,
    drag,
    switchLayout,
    pressKey,
    resolveRenderedKey,
    openVoiceKeyboard,
    closeKeyboard,
    beginPointerKey,
    endPointerKey,
    cancelPointerKey,
    stopKeyRepeat,
  } = useKeyboardPanel({ client, platform, layout, nineKeyShuangpinKeys });
  const { keySpacing: keyGap, rowSpacing: rowGap } = touchKeyboardSpacing(
    keySpacingTenths,
    rowSpacingTenths,
  );
  const keyboardStyle = {
    "--keyboard-key-gap": `${keyGap}px`,
    "--keyboard-row-gap": `${rowGap}px`,
  } as CSSProperties;
  const skinStyle = keyboardSkinStyles(theme, skin, customDesign);
  const keyMaterial = skin === "custom" ? (customDesign?.keyMaterial ?? "flat") : "flat";
  return (
    <main
      className={`native-panel ${surface.keyboardPanel}`}
      style={skinStyle}
      data-keyboard-theme={theme}
      data-keyboard-skin={skin}
      data-keyboard-material={keyMaterial}
      data-keyboard-layout={activeLayout}
      aria-label="屏幕键盘"
    >
      <NativePanelHeader
        className={surface.keyboardHeader}
        drag={drag}
        closeDisabled={openingVoice}
        onClose={closeKeyboard}
      >
        <span className={surface.keyboardNotice} role="status" title={notice}>
          {notice}
        </span>
        <button type="button" aria-label="切换键盘布局" onClick={switchLayout}>
          {platform === "windows" && activeLayout === "twenty_six_key"
            ? "十八键"
            : activeLayout === "twenty_six_key" || activeLayout === "eighteen_key"
              ? "九宫格"
              : "全键"}
        </button>
        {voiceShortcut && client.openVoice && (
          <button
            type="button"
            aria-label="打开语音输入"
            disabled={openingVoice}
            onClick={() => void openVoiceKeyboard()}
          >
            语音
          </button>
        )}
      </NativePanelHeader>
      <div className={surface.keyboardBody}>
        <div className={surface.keyboardLayout} data-keyboard-layout-grid="" style={keyboardStyle}>
          {rows.map((row, rowIndex) => (
            <div className={surface.keyboardRow} data-keyboard-row="" key={rowIndex}>
              {row.map((keyToRender, keyIndex) => (
                <KeyboardKeyButton
                  key={`${keyToRender.label}-${keyIndex}`}
                  keyToRender={keyToRender}
                  modifiers={renderedModifiers}
                  material={keyMaterial}
                  disabled={openingVoice}
                  flexGrow={
                    activeLayout === "nine_key" ||
                    activeLayout === "nine_key_shuangpin" ||
                    activeLayout === "eighteen_key"
                      ? 1
                      : keyboardKeyWeight(
                          keyToRender.label,
                          keyIndex,
                          row.some((item) => item.virtualKey === 0x20),
                        )
                  }
                  onPointerDown={beginPointerKey}
                  onPointerUp={endPointerKey}
                  onPointerCancel={cancelPointerKey}
                  onPointerLeave={stopKeyRepeat}
                  onActivate={(key, displayed) => pressKey(resolveRenderedKey(key, displayed))}
                />
              ))}
            </div>
          ))}
        </div>
      </div>
    </main>
  );
}

export function HandwritingPanel({
  client,
  theme = "dark",
  platform,
}: {
  client: PanelClient;
  theme?: "dark" | "light";
  /** The host platform. Only Windows recognises through a handwriting pack the user may be missing; elsewhere an empty result just means the strokes were not read. On Windows the stroke limit is also the shared contract's rather than the Linux provider's. */
  platform?: string;
}) {
  const {
    packs,
    handwritingPack,
    effectiveMode,
    selectActivation,
    strokes,
    redoStrokes,
    drawing,
    candidates,
    notice,
    drag,
    recognizing,
    closing,
    submitting,
    renderStrokes,
    closeHandwriting,
    start,
    move,
    end,
    cancel,
    undo,
    redo,
    clear,
    retryRecognition,
    editInk,
    navigateCandidates,
    chooseCandidate,
  } = useHandwritingPanel({ client, platform });
  return (
    <main
      className={surface.handwritingPanel}
      onKeyDown={editInk}
      data-panel-theme={theme}
      aria-label="手写识别板"
    >
      <NativePanelHeader
        title={
          <>
            <FluentIcon name="pen" size={18} />
            水杉手写识别板
          </>
        }
        className={surface.handwritingHeader}
        drag={drag}
        closeDisabled={closing}
        onClose={() => void closeHandwriting()}
      />
      <div className={surface.handwritingBody}>
        <HandwritingInkEditor
          strokes={renderStrokes}
          drawing={drawing.length > 0}
          disabled={closing}
          recognizing={recognizing}
          canUndo={strokes.length > 0 || drawing.length > 0}
          canRedo={redoStrokes.length > 0 && drawing.length === 0}
          canClear={strokes.length > 0 || drawing.length > 0 || redoStrokes.length > 0}
          canRecognize={strokes.length > 0 && drawing.length === 0 && !recognizing && !submitting}
          onStart={start}
          onMove={move}
          onEnd={end}
          onCancel={cancel}
          onUndo={undo}
          onRedo={redo}
          onClear={clear}
          onRecognize={client.recognizeHandwriting ? retryRecognition : undefined}
        />
        <HandwritingResults
          candidates={candidates}
          mode={effectiveMode}
          copyAvailable={Boolean(client.copyHandwritingCandidate)}
          inputAvailable={Boolean(client.submitHandwritingCandidate)}
          disabled={closing || submitting}
          notice={notice}
          onModeChange={selectActivation}
          onNavigate={navigateCandidates}
          onChoose={chooseCandidate}
        >
          {handwritingPack && <HandwritingModelNotice packs={packs} mirror={client.modelMirror} />}
        </HandwritingResults>
      </div>
    </main>
  );
}

export function VoicePanel({
  client,
  theme = "dark",
}: {
  client: VoicePanelClient;
  theme?: "dark" | "light";
}) {
  const {
    inputLevel,
    language,
    setLanguage,
    text,
    exceedsSubmitLimit,
    updateText,
    submitting,
    copying,
    busy,
    stopping,
    notice,
    drag,
    recognize,
    submit,
    stop,
    cancel,
    close,
    clear,
  } = useVoicePanel(client);

  return (
    <main
      className={`native-panel ${surface.voicePanel}`}
      data-panel-theme={theme}
      aria-label="语音输入"
    >
      <NativePanelHeader
        title="水杉语音输入"
        className={`native-panel-header ${surface.panelHeader}`}
        drag={drag}
        onClose={() => void close()}
      />
      <div className={surface.voiceBody}>
        <div className={surface.voiceIcon} aria-hidden="true">
          🎙
        </div>
        <h1>语音输入</h1>
        {busy && !stopping && inputLevel !== undefined && (
          <label>
            麦克风音量 <meter aria-label="麦克风音量" min={0} max={1} value={inputLevel} />
          </label>
        )}
        <p className={surface.voiceNote}>
          {client.description ??
            "录音和识别由已配置的 Linux provider 服务完成，输入法不会保存原始音频。"}
        </p>
        <label className={surface.voiceLanguage}>
          识别语言
          <input
            className={surface.voiceLanguageInput}
            value={language}
            maxLength={64}
            list="voice-language-options"
            onChange={(event) => setLanguage(event.target.value)}
            disabled={busy}
          />
          <datalist id="voice-language-options">
            <VoiceLanguageOptions />
          </datalist>
        </label>
        <button
          type="button"
          className={surface.voiceRecord}
          onClick={() => void (busy ? stop() : recognize())}
          disabled={submitting || stopping || (busy && !(client.stopVoice ?? client.cancelVoice))}
        >
          {stopping ? "正在完成识别…" : busy ? "停止录音" : "开始录音"}
        </button>
        {busy && client.stopVoice && client.cancelVoice && (
          <button type="button" aria-keyshortcuts="Escape" onClick={() => void cancel()}>
            取消录音
          </button>
        )}
        <VoiceResults
          text={text}
          exceedsSubmitLimit={exceedsSubmitLimit}
          inputAvailable={Boolean(client.sendVoiceText ?? client.sendText)}
          copyAvailable={Boolean(client.copyText)}
          busy={busy}
          submitting={submitting}
          copying={copying}
          onTextChange={updateText}
          onSubmit={submit}
          onClear={clear}
        />
        {busy && client.cancelVoice && <p className={surface.voiceNote}>按 Esc 取消录音</p>}
        <p className={surface.voiceNote} role="status">
          {notice}
        </p>
      </div>
    </main>
  );
}

export function CloudClipboardPanel({ client }: { client: CloudClipboardPanelClient }) {
  const {
    confirmation,
    setConfirmation,
    loaded,
    search,
    setSearch,
    items,
    draft,
    updateDraft,
    enabled,
    e2e,
    devices,
    showDevices,
    notice,
    busy,
    refresh,
    add,
    remove,
    deviceAction,
    toggle,
    confirm,
    toggleDevices,
    choose,
    pending,
    ready,
    waiting,
    waitingCount,
    confirmText,
  } = useCloudClipboardPanel(client);

  return (
    <main className={`native-panel ${cloud.clipboardPanel}`} aria-label="云剪贴板">
      <CloudPanelHeader title="水杉云剪贴板" onClose={() => void client.close()} />
      <div className={cloud.clipboardBody}>
        <p className={cloud.clipboardNote}>
          只上传你明确选择的文本，不自动读取本地剪贴板；内容在设备上加密后才上传，服务器只保存密文。
        </p>
        <label className={cloud.clipboardToggle}>
          <span>启用云剪贴板</span>
          <ControlledCheckbox
            checked={enabled}
            onChange={() => void toggle()}
            disabled={busy || !loaded}
          />
        </label>
        {confirmation && confirmText && (
          <div
            className={cloud.clipboardConfirm}
            role="alertdialog"
            aria-label={confirmText.label}
            aria-modal="true"
          >
            <p className="m-0">{confirmText.message}</p>
            <button
              type="button"
              className={cloud.clipboardDelete}
              disabled={busy}
              onClick={() => void confirm()}
            >
              {confirmText.action}
            </button>
            <button
              type="button"
              className={cloud.clipboardButton}
              onClick={() => setConfirmation(null)}
            >
              取消
            </button>
          </div>
        )}
        {pending && e2e ? (
          <CloudClipboardPendingNotice
            code={e2e.code}
            busy={busy}
            onRefresh={() => void refresh()}
            onReset={() => setConfirmation({ kind: "reset" })}
          />
        ) : (
          <>
            {ready && waitingCount > 0 && (
              <CloudClipboardWaitingDevices
                devices={waiting}
                count={waitingCount}
                busy={busy}
                onApprove={(device) =>
                  void deviceAction(
                    { operation: "approve", device_id: device.device_id },
                    `已允许「${device.name || "未命名设备"}」使用云剪贴板`,
                    "允许设备失败",
                  )
                }
                onReject={(device) =>
                  void deviceAction(
                    { operation: "reject", device_id: device.device_id },
                    `已拒绝「${device.name || "未命名设备"}」`,
                    "拒绝设备失败",
                  )
                }
                onShow={() => void refresh()}
              />
            )}
            {ready && (
              <button
                type="button"
                className={cloud.clipboardButton}
                aria-expanded={showDevices}
                disabled={busy}
                onClick={() => void toggleDevices()}
              >
                {showDevices ? "收起设备列表" : "管理设备"}
              </button>
            )}
            {ready && showDevices && devices && (
              <CloudClipboardDeviceList
                devices={devices.devices}
                busy={busy}
                onRemove={(device) => setConfirmation({ kind: "remove", device })}
                onReset={() => setConfirmation({ kind: "reset" })}
              />
            )}
            <div className={cloud.clipboardSearchRow}>
              <input
                className={cloud.clipboardInput}
                aria-label="搜索云端历史"
                value={search}
                onChange={(event) => {
                  setSearch(event.target.value);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void refresh();
                }}
                placeholder="搜索云端历史"
              />
              <button
                type="button"
                className={cloud.clipboardButton}
                onClick={() => void refresh()}
                disabled={busy}
              >
                刷新
              </button>
            </div>
            <CloudClipboardComposer
              draft={draft}
              enabled={enabled}
              busy={busy}
              onDraftChange={updateDraft}
              onUpload={add}
            />
            {e2e && e2e.undecryptable > 0 && (
              <p className={cloud.clipboardNote} role="note">
                有 {e2e.undecryptable}{" "}
                条云端记录无法在本设备解密，已略去。它们可能是用另一把密钥写入的。
              </p>
            )}
            <CloudClipboardHistory
              items={items}
              busy={busy}
              copyAvailable={!!client.copyText}
              onChoose={choose}
              onRemove={remove}
            />
          </>
        )}
        <p className={cloud.clipboardNote} role="status">
          {notice}
        </p>
      </div>
    </main>
  );
}

export function CloudDictionaryPanel({
  client,
  embedded = false,
}: {
  client: CloudDictionaryPanelClient;
  embedded?: boolean;
}) {
  const {
    confirmation,
    kind,
    search,
    setSearch,
    offset,
    entries,
    hasMore,
    loadError,
    form,
    notice,
    busy,
    refresh,
    beginAdd,
    beginEdit,
    save,
    confirmRemove,
    downloadToLocal,
    changeKind,
    changeForm,
    cancelForm,
  } = useCloudDictionaryPanel(client, embedded);
  return (
    <main className={`native-panel ${cloud.dictionaryPanel}`} aria-label="云词库">
      {confirmation}
      <CloudPanelHeader title="水杉云词库" onClose={() => void client.close()} />
      <div className={cloud.dictionaryBody}>
        <p className={cloud.dictionaryNote}>
          词条同步到当前账号，可在其他设备使用。应用到本机后用于输入。
        </p>
        <CloudDictionaryKindTabs value={kind} disabled={busy} onChange={changeKind} />
        <CloudDictionaryQueryToolbar
          kind={kind}
          busy={busy}
          queryLabel="搜索"
          queryAriaLabel="搜索云词条"
          queryValue={search}
          queryButtonLabel="查询"
          live={embedded}
          inputClassName={cloud.dictionaryInput}
          onKindChange={changeKind}
          onQueryChange={(value) => {
            setSearch(value);
          }}
          onQuery={() => void refresh(0)}
          placeholder="词条或编码"
        >
          <button type="button" onClick={beginAdd} disabled={busy}>
            添加词条
          </button>
          {client.openCatalog && (
            <button type="button" onClick={() => void client.openCatalog?.()} disabled={busy}>
              完整目录
            </button>
          )}
          {client.openCandidates && (
            <button type="button" onClick={() => void client.openCandidates?.()} disabled={busy}>
              云端候选排序
            </button>
          )}
          {client.openFiles && (
            <button type="button" onClick={() => void client.openFiles?.()} disabled={busy}>
              导入与导出
            </button>
          )}
          {client.openApply && client.snapshot && (
            <button type="button" onClick={() => void client.openApply?.()} disabled={busy}>
              应用到本机
            </button>
          )}
        </CloudDictionaryQueryToolbar>
        {form && (
          <CloudDictionaryEntryForm
            value={form}
            busy={busy}
            onChange={changeForm}
            onSave={() => void save()}
            onCancel={cancelForm}
          />
        )}
        {entries.length > 0 && (
          <p className={cloud.dictionaryMobileHint}>
            点按词条可编辑；窄屏下的下载和删除操作会分组显示。
          </p>
        )}
        <div className={cloud.dictionaryList} aria-label="云词条">
          {entries.length ? (
            entries.map((entry) => (
              <CloudDictionaryEntryCard
                key={entry.id}
                entry={entry}
                editLabel={`编辑云词条 ${entry.word}`}
                busy={busy}
                onEdit={() => beginEdit(entry)}
                onRemove={() => void confirmRemove(entry)}
                onDownload={client.downloadToLocal ? () => void downloadToLocal(entry) : undefined}
              />
            ))
          ) : (
            <p className={`${cloud.dictionaryEmpty} py-8`}>
              {busy ? "正在加载云词条…" : loadError ? "云词条加载失败" : "暂无云词条"}
            </p>
          )}
        </div>
        <CloudDictionaryPagination
          offset={offset}
          hasMore={hasMore}
          busy={busy}
          onPrevious={() => void refresh(Math.max(0, offset - 100))}
          onNext={() => void refresh(offset + 100)}
        />
        <p className={cloud.dictionaryNote} role="status">
          {notice}
        </p>
      </div>
    </main>
  );
}

/** File transfer lives on its own page so the dictionary list stays focused on entries. */
export function CloudDictionaryFilesPanel({ client }: { client: CloudDictionaryPanelClient }) {
  const {
    confirmation,
    kind,
    format,
    file,
    setFile,
    notice,
    busy,
    snapshotBusy,
    restorePreview,
    changeKind,
    setFormat,
    chooseFile,
    importFile,
    exportDictionary,
    exportSnapshot,
    chooseNativeRestoreSnapshot,
    chooseRestoreSnapshot,
    abandonRestoreSnapshot,
    restoreSnapshot,
  } = useCloudDictionaryFilesPanel(client);

  return (
    <main className={`native-panel ${cloud.dictionaryPanel}`} aria-label="云词库文件">
      {confirmation}
      <CloudPanelHeader
        title="导入与导出"
        backClassName={cloud.dictionaryButton}
        onBack={() => void (client.back ? client.back() : client.close())}
        onClose={() => void client.close()}
      />
      <div className={cloud.dictionaryBody}>
        <CloudDictionaryKindTabs value={kind} disabled={busy} onChange={changeKind} />
        <div className={cloud.dictionaryToolbar}>
          <label className={`${cloud.dictionaryField} ${cloud.dictionaryDesktopOnlyField}`}>
            词库
            <CloudDictionaryKindSelect value={kind} disabled={busy} onChange={changeKind} />
          </label>
          <CloudDictionarySelectField
            label="文件格式"
            ariaLabel="文件格式"
            labelClassName={cloud.dictionaryField}
            selectClassName={cloud.dictionaryInput}
            value={format}
            onChange={(value) => setFormat(value as CloudDictionaryFileFormat)}
            disabled={busy}
          >
            <DictionaryFormatOptions pinyin={kind === "pinyin"} />
          </CloudDictionarySelectField>
        </div>
        <p className={cloud.dictionaryNote}>
          导入只处理你明确选择的本地文件，读取和上传均有 64 KiB 边界；导出所选类型的云端个人词条。
        </p>
        <section className={cloud.dictionarySection} aria-label="导入云词库">
          <h2>导入云词库</h2>
          <label className="secondary">
            选择文件
            <input
              className={cloud.dictionaryInput}
              hidden
              type="file"
              accept=".txt,.tsv,text/plain"
              disabled={busy}
              onChange={(event) => {
                const selected = event.target.files?.[0];
                if (selected) chooseFile(selected);
                event.currentTarget.value = "";
              }}
            />
          </label>
          {file && (
            <div className={cloud.dictionaryFilePreview}>
              <strong>{file.name}</strong>
              <small>{file.bytes} 字节</small>
              <pre>{file.text.slice(0, 2000)}</pre>
              <button type="button" onClick={importFile} disabled={busy}>
                确认上传到云端
              </button>
              <button
                type="button"
                className="secondary"
                onClick={() => setFile(null)}
                disabled={busy}
              >
                取消
              </button>
            </div>
          )}
        </section>
        <section className={cloud.dictionarySection} aria-label="导出云词库">
          <h2>导出云词库</h2>
          <p className={cloud.dictionaryNote}>文件会下载到当前设备，不会修改云端词条。</p>
          <button type="button" onClick={exportDictionary} disabled={busy || format === "hans"}>
            导出当前类型
          </button>
        </section>
        {client.snapshot && (
          <section className={cloud.dictionarySection} aria-label="完整云词库备份">
            <h2>完整云词库备份</h2>
            <p className={cloud.dictionaryNote}>
              包含四类词库和排序记录；恢复只写云端，不会自动改动本机词库。
            </p>
            <button
              className={cloud.dictionaryButton}
              type="button"
              onClick={() => void exportSnapshot()}
              disabled={busy || snapshotBusy}
            >
              导出完整快照
            </button>
            {client.chooseSnapshotRestore ? (
              <button
                type="button"
                className="secondary"
                disabled={busy || snapshotBusy}
                onClick={() => void chooseNativeRestoreSnapshot()}
              >
                选择快照恢复到云端
              </button>
            ) : (
              <label className="secondary">
                选择快照恢复到云端
                <input
                  className={cloud.dictionaryInput}
                  hidden
                  type="file"
                  accept=".ndjson,application/x-ndjson"
                  disabled={busy || snapshotBusy}
                  onChange={(event) => {
                    const selected = event.target.files?.[0];
                    if (selected) void chooseRestoreSnapshot(selected);
                    event.currentTarget.value = "";
                  }}
                />
              </label>
            )}
            {restorePreview && (
              <div className={cloud.dictionaryFilePreview}>
                <strong>已校验快照</strong>
                <small>
                  云端 revision {restorePreview.snapshot.cloudRevision} ·{" "}
                  {restorePreview.snapshot.records} 条记录 · {restorePreview.snapshot.bytes} 字节
                </small>
                <button
                  className={cloud.dictionaryButton}
                  type="button"
                  onClick={() => void restoreSnapshot()}
                  disabled={busy || snapshotBusy}
                >
                  确认恢复云端词库
                </button>
                <button
                  type="button"
                  className="secondary"
                  onClick={() => void abandonRestoreSnapshot()}
                  disabled={busy || snapshotBusy}
                >
                  放弃恢复
                </button>
              </div>
            )}
          </section>
        )}
        <p className={cloud.dictionaryNote} role="status">
          {notice}
        </p>
      </div>
    </main>
  );
}

/** 通过宿主队列应用完整云词库快照。 */
export function CloudDictionaryApplyPanel({ client }: { client: CloudDictionaryPanelClient }) {
  const {
    confirmation,
    localVersion,
    request,
    preview,
    previewToken,
    notice,
    busy,
    refreshStatus,
    download,
    enqueue,
    cancel,
    discardPreview,
  } = useCloudDictionaryApplyPanel(client);
  return (
    <main className={`native-panel ${cloud.dictionaryPanel}`} aria-label="应用云词库">
      {confirmation}
      <CloudPanelHeader
        title="应用到本机"
        backClassName={cloud.dictionaryButton}
        onBack={() => void (client.back ? client.back() : client.close())}
        onClose={() => void client.close()}
      />
      <div className={cloud.dictionaryBody}>
        <CloudDictionaryApplyContent
          localVersion={localVersion}
          request={request}
          preview={preview}
          canApply={Boolean(previewToken)}
          busy={busy}
          appliesInPanel={Boolean(client.snapshotAppliesInPanel)}
          onDownload={download}
          onRefresh={refreshStatus}
          onEnqueue={enqueue}
          onDiscardPreview={discardPreview}
          onCancel={cancel}
        />
        <p className={cloud.dictionaryNote} role="status">
          {notice}
        </p>
      </div>
    </main>
  );
}

export function CloudDictionaryCatalogPanel({ client }: { client: CloudDictionaryPanelClient }) {
  const {
    confirmation,
    kind,
    code,
    setCode,
    scheme,
    setScheme,
    profile,
    setProfile,
    confirmed,
    entries,
    offset,
    hasMore,
    revision,
    normalized,
    form,
    notice,
    busy,
    queryCatalog,
    beginEdit,
    save,
    confirmRemove,
    changeKind,
    changeForm,
    cancelForm,
  } = useCloudDictionaryCatalogPanel(client);
  return (
    <main className={`native-panel ${cloud.dictionaryPanel}`} aria-label="完整云词库目录">
      {confirmation}
      <CloudPanelHeader
        title="完整云词库目录"
        backClassName={cloud.dictionaryButton}
        onBack={() => void (client.back ? client.back() : client.close())}
        onClose={() => void client.close()}
      />
      <div className={cloud.dictionaryBody}>
        <CloudDictionaryQueryToolbar
          kind={kind}
          busy={busy}
          queryLabel="编码"
          queryAriaLabel="完整目录编码"
          queryValue={code}
          queryButtonLabel="查询完整目录"
          queryDisabled={kind !== "quick" && !code.trim()}
          inputClassName={cloud.dictionaryInput}
          onKindChange={changeKind}
          onQueryChange={setCode}
          onQuery={() => void queryCatalog()}
          placeholder={kind === "quick" ? "可留空" : "例如 shi、a 或 hello"}
          queryButtonClassName={cloud.dictionaryButton}
        />
        {kind === "pinyin" && (
          <div className={cloud.dictionaryActions}>
            <CloudDictionarySelectField
              label="编码方案"
              ariaLabel="编码方案"
              labelClassName={cloud.dictionaryField}
              selectClassName={cloud.dictionaryInput}
              value={scheme}
              onChange={setScheme}
              disabled={busy}
            >
              <CloudPinyinSchemeOptions />
            </CloudDictionarySelectField>
            {scheme === "shuangpin" && (
              <CloudDictionarySelectField
                label="双拼方案"
                ariaLabel="双拼方案"
                labelClassName={cloud.dictionaryField}
                selectClassName={cloud.dictionaryInput}
                value={profile}
                onChange={setProfile}
                disabled={busy}
              >
                <CloudShuangpinProfileOptions />
              </CloudDictionarySelectField>
            )}
          </div>
        )}
        <p className={cloud.dictionaryNote}>
          包含基础词库与当前账号修改。编辑和删除只影响云端目录，不会自动修改本机词库。
        </p>
        {confirmed && entries.length > 0 && (
          <p className={cloud.dictionaryMobileHint}>点按词条可编辑；窄屏下的删除操作会分组显示。</p>
        )}
        {confirmed && (
          <div className={cloud.dictionaryList} aria-label="完整目录结果">
            <p>
              查询编码：{normalized} · 云端版本 {revision}
            </p>
            {entries.length ? (
              entries.map((entry) => (
                <CloudDictionaryEntryCard
                  key={`${entry.kind}:${entry.code}:${entry.word}`}
                  entry={entry}
                  editLabel={`编辑完整目录词条 ${entry.word}`}
                  busy={busy}
                  onEdit={() => beginEdit(entry)}
                  onRemove={() => void confirmRemove(entry)}
                />
              ))
            ) : (
              <p className={cloud.dictionaryEmpty}>没有匹配的词条</p>
            )}
          </div>
        )}
        {form && (
          <CloudDictionaryEntryForm
            value={form}
            busy={busy}
            onChange={changeForm}
            onSave={() => void save()}
            onCancel={cancelForm}
          />
        )}
        {confirmed && (
          <CloudDictionaryPagination
            offset={offset}
            hasMore={hasMore}
            busy={busy}
            onPrevious={() => void queryCatalog(Math.max(0, offset - 100), confirmed)}
            onNext={() => void queryCatalog(offset + 100, confirmed)}
          />
        )}
        <p className={cloud.dictionaryNote} role="status">
          {notice}
        </p>
      </div>
    </main>
  );
}

const cloudRankingModes: [CloudRankingMode, string][] = [
  ["disabled", "不调频"],
  ["pin", "置顶调频"],
  ["halve", "折半调频"],
  ["linear", "线性调频"],
  ["promote", "一次置前"],
];

export function CloudCandidatesPanel({ client }: { client: CloudDictionaryPanelClient }) {
  const {
    confirmation,
    kind,
    text,
    setText,
    scheme,
    setScheme,
    profile,
    setProfile,
    jianpin,
    setJianpin,
    mode,
    setMode,
    step,
    setStep,
    trigger,
    setTrigger,
    forceTop,
    setForceTop,
    position,
    setPosition,
    query,
    candidates,
    positions,
    revision,
    notice,
    busy,
    queryCandidates,
    rank,
    remove,
    setFixed,
    unfix,
    changeKind,
  } = useCloudCandidatesPanel(client);

  return (
    <main className={`native-panel ${cloud.dictionaryPanel}`} aria-label="云端候选排序">
      {confirmation}
      <CloudPanelHeader
        title="云端候选排序"
        onBack={() => void (client.back ? client.back() : client.close())}
        onClose={() => void client.close()}
      />
      <div className={cloud.dictionaryBody}>
        <CloudDictionaryQueryToolbar
          kind={kind}
          busy={busy}
          queryLabel="编码"
          queryAriaLabel="云端候选编码"
          queryValue={text}
          queryButtonLabel="查询云端候选"
          queryDisabled={!text.trim()}
          onKindChange={changeKind}
          onQueryChange={(value) => {
            setText(value);
          }}
          onQuery={() => void queryCandidates()}
        />
        {kind === "pinyin" && (
          <div className={cloud.dictionaryActions}>
            <label>
              <ControlledCheckbox checked={jianpin} onChange={setJianpin} disabled={busy} />
              简拼候选
            </label>
            <CloudDictionarySelectField
              label="编码方案"
              ariaLabel="候选编码方案"
              value={scheme}
              onChange={setScheme}
              disabled={busy}
            >
              <CloudPinyinSchemeOptions />
            </CloudDictionarySelectField>
            {scheme === "shuangpin" && (
              <CloudDictionarySelectField
                label="双拼方案"
                ariaLabel="候选双拼方案"
                value={profile}
                onChange={setProfile}
                disabled={busy}
              >
                <CloudShuangpinProfileOptions />
              </CloudDictionarySelectField>
            )}
          </div>
        )}
        {kind !== "quick" && (
          <div className={cloud.dictionaryActions}>
            <label>
              调频方式
              <select
                aria-label="调频方式"
                value={mode}
                onChange={(event) => setMode(event.target.value as CloudRankingMode)}
                disabled={busy}
              >
                {cloudRankingModes.map(([value, label]) => (
                  <option key={value} value={value}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              线性步长
              <input
                aria-label="线性步长"
                type="number"
                min="1"
                max="100"
                value={step}
                onChange={(event) => setStep(clamp(Number(event.target.value) || 1, 1, 100))}
                disabled={busy || mode !== "linear"}
              />
            </label>
            <label>
              触发次数
              <input
                aria-label="触发次数"
                type="number"
                min="1"
                max="10"
                value={trigger}
                onChange={(event) => setTrigger(clamp(Number(event.target.value) || 1, 1, 10))}
                disabled={busy}
              />
            </label>
            <label>
              <ControlledCheckbox checked={forceTop} onChange={setForceTop} disabled={busy} />
              强制置顶
            </label>
            <label>
              固定位置
              <select
                aria-label="固定位置"
                value={position}
                onChange={(event) => setPosition(Number(event.target.value))}
                disabled={busy}
              >
                {[1, 2, 3, 4, 5].map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
            </label>
          </div>
        )}
        <p className={cloud.dictionaryNote}>
          排序、固定位置和删除只保存到当前账号；本机键盘需要后续同步才会采用云端状态。
        </p>
        {query && (
          <CloudCandidateResults
            kind={kind}
            queryText={query.text}
            revision={revision}
            candidates={candidates}
            busy={busy}
            position={position}
            onRank={rank}
            onSetFixed={setFixed}
            onRemove={remove}
          />
        )}
        <CloudFixedPositions items={positions} busy={busy} onUnfix={unfix} />
        <p className={cloud.dictionaryNote} role="status">
          {notice}
        </p>
      </div>
    </main>
  );
}

const emojiPages: { id: EmojiPage; label: string; icon: string }[] = [
  { id: "home", label: "最近使用", icon: "◷" },
  { id: "emoji", label: "Emoji", icon: "😀" },
  { id: "sticker", label: "贴纸", icon: "🖼" },
  { id: "gif", label: "GIF", icon: "GIF" },
  { id: "kaomoji", label: "颜文字", icon: "ヾ" },
  { id: "symbols", label: "符号", icon: "★" },
  { id: "clipboard", label: "剪贴板", icon: "▣" },
];

/*
 * The emoji panel is its own window with its own palette, switched by `data-panel-theme` on the root
 * rather than by the app's theme variables -- it has to match the host's panel chrome, not the
 * settings page. Every colour therefore comes in a dark/light pair, and the stylesheet wrote each
 * one twice: once as a base rule and again under a `[data-panel-theme="light"]` descendant selector,
 * thirty rules whose only job was to restate the first thirty. The pairs are named once here and the
 * light half rides along as a `group-data` variant, so a colour that changes changes in one place.
 */
const panelDim = "text-[#b8b8c0] group-data-[panel-theme=light]:text-[#656a73]";
const panelDivider = "border-white/[0.09] group-data-[panel-theme=light]:border-[#d9dce3]";
/** What a hoverable control settles on: a lift in the background and full-strength text. */
const panelRaise =
  "hover:bg-[#303038] hover:text-[#f5f5f7] group-data-[panel-theme=light]:hover:bg-[#eceef3] group-data-[panel-theme=light]:hover:text-[#202124]";
/** The selected state of a tab, a category chip or an activation choice. */
const panelChosen =
  "bg-[#303038] text-[#f5f5f7] group-data-[panel-theme=light]:bg-[#eceef3] group-data-[panel-theme=light]:text-[#202124]";
const panelAccentBorder = "border-[#5fbf84] group-data-[panel-theme=light]:border-[#2c7a4b]";
/** A quiet text button: the back link, a toolbar action, a group's trailing action. */
const panelTextButton = `rounded-[5px] border-0 bg-transparent ${panelDim} ${panelRaise}`;
const panelToolbar = `mb-2 flex min-h-[34px] items-center gap-[9px] border-b pb-2.5 ${panelDivider}`;
/** The size a quiet button takes in a toolbar. Each utility is written out somewhere as a literal --
 * Tailwind scans source text, so a class name assembled from string parts at runtime is one it never
 * generates. Interpolating a constant that already holds literals is fine; building `[&>button]:` +
 * a name is not. */
const panelAction = `${panelTextButton} px-2.5 py-[5px] text-xs`;
const panelHeading = `m-0 text-sm font-semibold ${surface.panelText}`;
const panelContent =
  "min-h-0 flex-1 overflow-y-auto px-6 pt-2 pb-[52px] max-phone:px-3.5 [scrollbar-color:#77747d_transparent]";
const panelChip = (chosen: boolean) =>
  `shrink-0 rounded-[7px] border px-2.5 py-1.5 font-[inherit] text-xs ${
    chosen
      ? `${panelAccentBorder} ${panelChosen}`
      : `border-transparent bg-transparent ${panelDim} ${panelRaise} focus-visible:bg-[#303038] focus-visible:text-[#f5f5f7] group-data-[panel-theme=light]:focus-visible:bg-[#eceef3] group-data-[panel-theme=light]:focus-visible:text-[#202124]`
  }`;
const panelEmpty = `m-0 flex min-h-[220px] items-center justify-center text-center text-sm ${surface.panelMuted}`;
/** 临时消息的胶囊提示，尺寸和配色取自 macOS 原生表情面板的 `MacEmojiToastOverlay`：浮在底部状态行上方，不接收指针。 */
const panelToast =
  "pointer-events-none absolute bottom-[42px] left-1/2 z-10 flex h-[35px] max-w-[calc(100%-27px)] -translate-x-1/2 items-center overflow-hidden rounded-full bg-[#3a3a44]/96 px-[19px] text-[13px] font-semibold whitespace-nowrap text-[#f5f5f7] group-data-[panel-theme=light]:bg-[#2b2b33]/94";
/** 剪贴板采集状态，文案与 macOS 原生剪贴板页的状态行一致；Linux 另有读不到系统剪贴板的三种，按当前会话该装什么来说。 */
const captureStatusText: Record<ClipboardCaptureStatus, string> = {
  disabled: "剪贴板采集已停用",
  monitoring: "历史开启时，保存新复制的文本",
  unavailable: "无法读取共享设置，剪贴板采集已暂停",
  read_unavailable: "当前会话里读不到系统剪贴板，剪贴板采集已暂停",
  read_unavailable_x11: "读不到系统剪贴板，剪贴板采集已暂停：请安装 xclip 或 xsel",
  read_unavailable_wayland: "读不到系统剪贴板，剪贴板采集已暂停：请安装 wl-clipboard",
  save_failed: "剪贴板历史保存失败，将重试",
};
export function EmojiPanel({
  client,
  theme = "dark",
  initialPage = "home",
  standaloneClipboard = false,
}: {
  client: EmojiPanelClient;
  theme?: "dark" | "light";
  initialPage?: "home" | "clipboard";
  /** 独立的本地剪贴板窗口（Linux 的「水杉本地剪贴板」）：页头叫「剪贴板」，不显示表情面板的分类导航。 */
  standaloneClipboard?: boolean;
}) {
  const {
    page,
    itemPage,
    query,
    recent,
    symbolRecent,
    clipboard,
    clipboardBusy,
    clipboardLoadFailed,
    clipboardEnabled,
    captureStatus,
    notice,
    toast,
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
    sendClipboardToCloud,
    removeClipboard,
    clearClipboard,
    clearRecent,
    closeEmoji,
    updateQuery,
    retryCatalog,
    selectActivation,
    clearSymbolRecent,
    selectSymbolCategory,
    drag,
    resize,
    refreshClipboard,
    showMore,
    previousItemPage,
    nextItemPage,
  } = useEmojiPanel(client, initialPage, standaloneClipboard);
  const selectCatalogItem = (item: EmojiCatalogItem) => copy(item.text, false, item.keywords);
  const standalone = standaloneClipboard && page === "clipboard";
  return (
    <main
      {...navigation}
      onPointerDownCapture={resize}
      // `panelSurface` 的浅色变体是 `group-data`，只作用于子元素；根元素自己就是这个 group，浅色底色要直接按自身的 `data-panel-theme` 给，否则浅色主题下空白处仍是深色。
      className={`native-panel group relative flex min-h-screen flex-col overflow-hidden ${page === "symbols" || client.resizeWindow ? "h-screen" : ""} ${surface.panelSurface} ${surface.panelText} data-[panel-theme=light]:bg-[#f7f8fa] data-[panel-theme=light]:text-[#202124]`}
      data-panel-theme={theme}
      aria-label={standalone ? "剪贴板" : "表情与符号"}
    >
      <NativePanelHeader
        title={standalone ? "剪贴板" : page === "symbols" ? "符号大全" : "Emoji and more"}
        className={`native-panel-header flex-[0_0_38px] border-b ${panelDivider} ${surface.panelSurface} ${surface.panelText} [&>button]:text-[#aeb0b7] group-data-[panel-theme=light]:[&>button]:text-[#656a73]`}
        closeDisabled={clipboardBusy}
        drag={drag}
        onClose={() => void closeEmoji()}
      />
      <div
        className={`emoji-panel-search mx-6 mt-3.5 flex ${page === "symbols" ? "flex-[0_0_40px]" : "flex-[0_0_52px]"} items-center gap-2 rounded-[10px] px-3.5 max-phone:mx-3.5 ${panelField} focus-within:border-[#5fbf84] focus-within:shadow-[0_0_0_1px_rgba(95,191,132,0.35)] group-data-[panel-theme=light]:focus-within:border-[#2c7a4b] group-data-[panel-theme=light]:focus-within:shadow-[0_0_0_1px_rgba(154,98,173,0.28)]`}
      >
        <span className="text-[23px] leading-none" aria-hidden="true">
          ⌕
        </span>
        <input
          className="min-w-0 flex-1 border-0 bg-transparent font-[inherit] text-[#f5f5f7] outline-0 placeholder:text-[#aeb0b7] group-data-[panel-theme=light]:text-[#202124] group-data-[panel-theme=light]:placeholder:text-[#7a7e87]"
          data-panel-search=""
          aria-label="搜索"
          aria-keyshortcuts="Control+f"
          value={query}
          onChange={(event) => updateQuery(event.target.value)}
          placeholder={
            page === "clipboard"
              ? "搜索剪贴板"
              : page === "emoji"
                ? "Search emojis"
                : page === "kaomoji"
                  ? "Search kaomoji"
                  : page === "symbols"
                    ? "搜索符号、名称或分类"
                    : page === "home"
                      ? "Search emoji, kaomoji, and symbols"
                      : "Search"
          }
        />
      </div>
      <nav
        className={`flex ${page === "symbols" ? "flex-[0_0_38px]" : "flex-[0_0_72px]"} items-stretch gap-1 px-[18px] pt-2 max-phone:px-2`}
        aria-label="面板分类"
        hidden={standalone}
      >
        {emojiPages.map((item) => (
          <button
            type="button"
            key={item.id}
            className={`flex min-w-0 flex-1 flex-col items-center justify-center gap-[3px] rounded-t-[7px] border-0 bg-transparent ${
              page === item.id
                ? `${panelChosen} shadow-[inset_0_-3px_#5fbf84] group-data-[panel-theme=light]:shadow-[inset_0_-3px_#2c7a4b]`
                : `${panelDim} ${panelRaise}`
            }`}
            aria-label={item.label}
            aria-pressed={page === item.id}
            onClick={() => selectPage(item.id)}
          >
            {page !== "symbols" && (
              <span className="min-h-7 font-emoji text-[23px] leading-[1.1]" aria-hidden="true">
                {item.icon}
              </span>
            )}
            <small className="m-0 text-[10px] leading-[1.2] text-inherit max-phone:text-[9px]">
              {item.label}
            </small>
          </button>
        ))}
      </nav>
      {isDetail && page !== "symbols" && !standalone && (
        <div className="flex-[0_0_32px] px-6">
          <button
            type="button"
            className={`${panelTextButton} px-2 py-[5px]`}
            aria-label="返回"
            onClick={() => selectPage("home")}
          >
            ‹ 返回
          </button>
        </div>
      )}
      {page !== "clipboard" &&
        page !== "sticker" &&
        page !== "gif" &&
        catalogUnavailable.length > 0 && (
          <div
            className={`${panelToolbar} shrink-0 grow-0 basis-auto px-6 py-1.5 text-xs`}
            role="status"
            aria-busy={catalogLoading}
          >
            <span>
              {catalogUnavailable
                .map((kind) => ({ emoji: "Emoji", kaomoji: "颜文字", symbols: "符号" })[kind])
                .join("、")}
              目录加载失败，暂用已有目录
            </span>
            <button
              type="button"
              className={`${panelAction} ml-auto`}
              disabled={catalogLoading || clipboardBusy}
              onClick={retryCatalog}
            >
              {catalogLoading ? "正在加载…" : "重新加载"}
            </button>
          </div>
        )}
      {page !== "clipboard" &&
        page !== "sticker" &&
        page !== "gif" &&
        canCopy &&
        client.sendText && (
          <div
            className={`flex shrink-0 items-center gap-1.5 px-6 py-1 text-xs ${panelDim}`}
            role="group"
            aria-label="点击项目时的操作"
          >
            <span>点击项目：</span>
            <PanelActivationButtons
              mode={effectiveMode}
              disabled={clipboardBusy}
              inputAvailable
              inputLabel="输入到原应用"
              onModeChange={selectActivation}
              buttonClassName={(selected) =>
                `rounded-md border px-[9px] py-[5px] font-[inherit] disabled:cursor-wait disabled:opacity-55 ${
                  selected
                    ? `${panelAccentBorder} ${panelChosen}`
                    : "border-transparent bg-transparent text-inherit"
                }`
              }
            />
          </div>
        )}
      {categoryPage && page !== "symbols" && (
        <nav
          className="flex shrink-0 gap-1.5 overflow-x-auto px-6 py-2 whitespace-pre-wrap"
          aria-label={page === "emoji" ? "Emoji 子分类" : "符号子分类"}
        >
          <button
            type="button"
            className={panelChip(activeCategory === "all")}
            aria-pressed={activeCategory === "all"}
            onClick={() => selectCategory("all")}
          >
            全部
          </button>
          {page === "emoji" && (
            <button
              type="button"
              className={panelChip(activeCategory === "recent")}
              aria-pressed={activeCategory === "recent"}
              onClick={() => selectCategory("recent")}
            >
              ◷ 最近使用
            </button>
          )}
          {categoryTabs.map((tab) => (
            <button
              type="button"
              key={tab.id}
              className={panelChip(activeCategory === tab.id)}
              aria-pressed={activeCategory === tab.id}
              onClick={() => selectCategory(tab.id)}
            >
              <span aria-hidden="true">{tab.icon}</span> {tab.title}
            </button>
          ))}
        </nav>
      )}
      {page === "symbols" ? (
        <div className="flex min-h-0 flex-1 flex-col">
          <section
            className={`mx-6 mt-2 border-b pb-2 ${panelDivider}`}
            aria-label="最近使用的符号"
          >
            <div className="flex items-center gap-2 text-xs">
              <span>最近使用</span>
              {symbolRecent.length > 0 && (
                <button
                  type="button"
                  className={`${panelAction} ml-auto`}
                  onClick={clearSymbolRecent}
                >
                  清除符号记录
                </button>
              )}
            </div>
            <div className="mt-1 flex gap-1 overflow-x-auto">
              {symbolRecent.length ? (
                symbolRecent.map((item) => (
                  <EmojiItemButton
                    key={item.text}
                    item={item}
                    compact
                    className="shrink-0 min-w-10"
                    onSelect={selectCatalogItem}
                  />
                ))
              ) : (
                <p className={`my-1 text-xs ${panelDim}`}>使用过的符号会显示在这里</p>
              )}
            </div>
          </section>
          <div className="flex min-h-0 flex-1 pt-2">
            <nav
              className={`w-36 shrink-0 overflow-y-auto border-r px-2 pb-3 ${panelDivider}`}
              aria-label="符号子分类"
            >
              {[{ id: "all", title: "全部符号", icon: "" }, ...categoryTabs].map((tab) => (
                <button
                  type="button"
                  key={tab.id}
                  className={`${panelChip(activeCategory === tab.id)} mb-1 w-full text-left`}
                  aria-pressed={activeCategory === tab.id}
                  onClick={() => selectSymbolCategory(tab.id)}
                >
                  {tab.title}
                </button>
              ))}
            </nav>
            <section
              className="emoji-panel-content min-h-0 flex-1 overflow-y-auto px-3 pt-2 pb-3 [scrollbar-color:#77747d_transparent]"
              aria-label="符号目录"
            >
              {filteredGroups.map((group) => (
                <div key={JSON.stringify([group.pack, group.parent, group.title])} className="mb-3">
                  <h2 className={`${panelHeading} mb-2`}>
                    {group.pack ? group.title : symbolCategoryTitle(group.title)}
                  </h2>
                  <div className="grid grid-cols-[repeat(auto-fill,minmax(36px,1fr))] gap-1">
                    {group.items.map((item, index) => (
                      <EmojiItemButton
                        key={`${index}-${item.text}`}
                        item={item}
                        compact
                        navigable
                        onSelect={selectCatalogItem}
                      />
                    ))}
                  </div>
                </div>
              ))}
              {!filteredGroups.length && (
                <p className={panelEmpty}>
                  {catalogLoading ? "正在加载符号…" : query ? "没有匹配的符号" : "暂无可显示符号"}
                </p>
              )}
            </section>
          </div>
        </div>
      ) : page === "clipboard" ? (
        <section className={`${panelContent} pt-3.5`} aria-label="剪贴板历史">
          <div className={panelToolbar}>
            <h2 className={panelHeading}>剪贴板</h2>
            <div className="ml-auto flex gap-2">
              {client.clipboard?.list && (
                <button
                  type="button"
                  className={panelAction}
                  disabled={clipboardBusy}
                  onClick={refreshClipboard}
                >
                  刷新列表
                </button>
              )}
              {client.clipboard?.sync && (
                <button
                  type="button"
                  className={panelAction}
                  disabled={clipboardBusy || clipboardEnabled === false}
                  onClick={() => void syncClipboard()}
                >
                  同步
                </button>
              )}
              {client.clipboard?.clear && (
                <button
                  type="button"
                  className={panelAction}
                  disabled={clipboardBusy || !clipboard.length}
                  onClick={() => void clearClipboard()}
                >
                  清空历史
                </button>
              )}
            </div>
          </div>
          {captureStatus && (
            <p className={`m-0 mb-2 text-xs ${panelDim}`} data-clipboard-capture={captureStatus}>
              {captureStatusText[captureStatus]}
            </p>
          )}
          {clipboardLoadFailed ? (
            <p className={panelEmpty} role="status">
              无法读取剪贴板历史，请点击“刷新列表”重试
            </p>
          ) : clipboardEnabled === false ? (
            <div className={`${panelEmpty} flex-col gap-2`}>
              <p>剪贴板历史已关闭</p>
              <p>开启后保存复制过的文本；关闭会清空历史。</p>
              {client.clipboard?.enable && (
                <button
                  type="button"
                  className={panelAction}
                  disabled={clipboardBusy}
                  onClick={() => void enableClipboard()}
                >
                  开启剪贴板历史
                </button>
              )}
            </div>
          ) : visibleClipboard.length ? (
            <EmojiClipboardEntries
              items={visibleClipboard}
              busy={clipboardBusy}
              canCopy={canCopy}
              onCopy={(text) => void copy(text, true)}
              onItemKeyDown={clipboardItemKey}
              onPaste={client.clipboard?.paste ? pasteClipboard : undefined}
              onSendToCloud={client.clipboard?.sendToCloud ? sendClipboardToCloud : undefined}
              onRemove={client.clipboard?.remove ? removeClipboard : undefined}
            />
          ) : (
            <p className={panelEmpty} role="status">
              {query ? "没有匹配的剪贴板记录" : "暂无剪贴板记录"}
            </p>
          )}
        </section>
      ) : page === "sticker" || page === "gif" ? (
        <p className={panelEmpty}>
          {page === "sticker" ? "贴纸来源可在这里接入" : "GIF 来源可在这里接入"}
        </p>
      ) : (
        <section
          className={panelContent}
          aria-label={
            page === "home"
              ? "最近使用与目录"
              : page === "emoji"
                ? "Emoji 目录"
                : page === "kaomoji"
                  ? "颜文字目录"
                  : "符号目录"
          }
        >
          {(page === "home" || (page === "emoji" && activeCategory === "recent")) &&
            recent.length > 0 && (
              <div className={panelToolbar}>
                <span>最近使用</span>
                <button
                  type="button"
                  className={`${panelAction} ml-auto`}
                  onClick={clearRecent}
                  disabled={clipboardBusy}
                >
                  清除最近使用
                </button>
              </div>
            )}
          {displayGroups.map((group) => (
            <div
              className="mb-[22px]"
              key={JSON.stringify([group.pack ?? null, group.parent ?? null, group.title])}
            >
              <div className="mb-2 flex min-h-[34px] items-center gap-[9px]">
                <span className="w-6 text-center font-emoji text-xl">{group.icon}</span>
                <h2 className={panelHeading}>{group.title}</h2>
                {group.moreTarget && (
                  <button
                    type="button"
                    className={`${panelAction} ml-auto`}
                    onClick={() => showMore(group.moreTarget!)}
                  >
                    更多
                  </button>
                )}
              </div>
              <div
                className={
                  group.flow
                    ? "flex flex-wrap items-stretch gap-1.5"
                    : "grid grid-cols-6 gap-1.5 max-phone:grid-cols-4"
                }
              >
                {group.items.map((item, index) => (
                  <EmojiItemButton
                    item={item}
                    flow={group.flow === true}
                    navigable
                    disabled={clipboardBusy}
                    key={`${index}-${item.text}`}
                    onSelect={selectCatalogItem}
                  />
                ))}
              </div>
            </div>
          ))}
          {catalogLoading && (
            <p className={`${panelEmpty} opacity-75`} role="status" aria-live="polite">
              正在加载表情库…
            </p>
          )}
          {!displayGroups.length && !catalogLoading && (
            <p className={panelEmpty}>
              {page === "emoji" && activeCategory === "recent" && !recent.length
                ? "Your recently used items will appear here"
                : query
                  ? "No results"
                  : "暂无可显示内容"}
            </p>
          )}
          {itemPageCount > 1 && (
            <div className={panelToolbar} role="navigation" aria-label="Emoji 分页">
              <button
                type="button"
                className={panelAction}
                disabled={itemPage === 0}
                onClick={previousItemPage}
              >
                上一页
              </button>
              <span>
                第 {itemPage + 1} / {itemPageCount} 页
              </span>
              <button
                type="button"
                disabled={itemPage + 1 >= itemPageCount}
                className={panelAction}
                onClick={nextItemPage}
              >
                下一页
              </button>
            </div>
          )}
        </section>
      )}
      <p
        className={`m-0 min-h-[30px] flex-[0_0_30px] border-t px-6 py-1.5 text-center text-xs ${panelDivider} ${surface.panelMuted}`}
        role="status"
      >
        {notice ||
          (page === "clipboard" || effectiveMode === "copy"
            ? "点击项目即可复制"
            : "点击项目即可输入到原应用")}
      </p>
      {client.resizeWindow && (
        <div
          aria-hidden="true"
          title="拖动边缘调整大小"
          className="absolute right-0 bottom-0 size-2 cursor-se-resize"
        >
          <svg viewBox="0 0 8 8" fill="none" stroke="currentColor">
            <path d="M1 7 7 1 M5 7 7 5" />
          </svg>
        </div>
      )}
      {toast && (
        <div className={panelToast} role="status" data-panel-toast="">
          {/* whitespace-pre 保留「已复制」后面的两个空格，与 macOS 原生提示一致；nowrap 会把它们并成一个。 */}
          <span className="overflow-hidden text-ellipsis whitespace-pre">{toast}</span>
        </div>
      )}
    </main>
  );
}
