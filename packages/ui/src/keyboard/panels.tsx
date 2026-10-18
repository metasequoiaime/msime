import { useEmojiPanel, type EmojiPage } from "./use-emoji-panel";
import { useCloudDictionaryFilesPanel } from "./use-cloud-dictionary-files-panel";
import { useCloudCandidatesPanel } from "./use-cloud-candidates-panel";
import { useCloudDictionaryCatalogPanel } from "./use-cloud-dictionary-catalog-panel";
import { useCloudDictionaryApplyPanel } from "./use-cloud-dictionary-apply-panel";
import { useCloudDictionaryPanel } from "./use-cloud-dictionary-panel";
import { useCloudClipboardPanel } from "./use-cloud-clipboard-panel";
import { useVoicePanel } from "./use-voice-panel";
import { isComposingKeyEvent } from "../core/ime-key-event";
import type {
  KeyboardInputRequest,
  PanelClient,
  VoicePanelClient,
  CloudClipboardPanelClient,
  CloudRankingMode,
  CloudDictionaryFileFormat,
  CloudDictionaryPanelClient,
  EmojiPanelClient,
} from "./panel-client";
import { FluentIcon } from "../core/fluent-icons";
import { ControlledCheckbox } from "../core/controlled-checkbox";
import { subscribeToElementResize } from "../core/element-resize";
import { subscribeToWindowBlur, subscribeToWindowEvent } from "../core/window-focus";
import { readOptionalStorage, writeOptionalStorage } from "../core/optional-storage";
import { clamp } from "../core/number";
import { touchKeyboardSpacing } from "../settings/touch-keyboard-spacing";
import { DictionaryFormatOptions } from "../dictionary/dictionary-format-options";
import { usePanelDrag } from "./use-panel-drag";
import { useHandwritingPanel } from "./use-handwriting-panel";
import { useEffect, useRef, useState, type CSSProperties, type PointerEvent } from "react";
import type { GlobalTheme } from "../theme/global-theme";
import * as cloud from "./cloud-panel-style";
import * as surface from "./panel-surface-style";
import { StatusMessage } from "../core/status-message";
import { VoiceLanguageOptions } from "../voice/voice-language-options";
import { useMountedRef } from "../settings/use-mounted-ref";
import { useLatestRef } from "../core/use-latest-ref";
import {
  isImeCommitKey,
  keyboardKeyWeight,
  modifierPrefix,
  type KeyboardKey,
  type Modifier,
} from "./keyboard-input";
import { keyboardSkinStyles } from "./keyboard-skin-styles";
import { keyboardDeliveryNotice, MACOS_KEYBOARD_GUIDANCE } from "./keyboard-delivery-notice";
import { keyboardRows, nineKeyRows, nineKeyShuangpinRows } from "./panel-keyboard-layouts";
import type { NineKeyShuangpinKeyFace } from "../settings/nine-key-shuangpin-profile";
import { CloudDictionaryEntryForm } from "./cloud-dictionary-entry-form";
import { CloudDictionaryEntryCard } from "./cloud-dictionary-entry-card";
import { CloudDictionaryItem } from "./cloud-dictionary-item";
import { CloudPanelHeader } from "./cloud-panel-header";
import { CloudDictionaryPagination } from "./cloud-dictionary-pagination";
import { NativePanelHeader } from "./native-panel-header";
import { PanelActivationButtons } from "./panel-activation-buttons";
import { CloudDictionaryKindTabs } from "./cloud-dictionary-kind-tabs";
import { CloudDictionaryKindSelect } from "./cloud-dictionary-kind-select";
import { CloudDictionarySelectField } from "./cloud-dictionary-select-field";
import { CloudDictionaryQueryToolbar } from "./cloud-dictionary-query-toolbar";
import { CloudPinyinSchemeOptions, CloudShuangpinProfileOptions } from "./cloud-scheme-options";
import { clipboardTooltip, emojiDisplayName, symbolCategoryTitle } from "./emoji-panel-helpers";

export { emojiDisplayName } from "./emoji-panel-helpers";
import {
  CloudClipboardDeviceList,
  CloudClipboardPendingNotice,
  CloudClipboardWaitingDevices,
} from "./cloud-clipboard-devices";
import { candidateMutationCode } from "./cloud-response";
import type { TouchKeyboardSkinDesign } from "./touch-keyboard-skin-design";
import {
  resourcePackStatus,
  type ModelMirrorClient,
  type ResourcePacks,
} from "../settings/resource-packs";
import {
  formatModelBytes,
  localModelProgressPercent,
  validModelMirror,
} from "../voice/local-model-helpers";

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
  const [activeLayout, setActiveLayout] = useState<
    "twenty_six_key" | "nine_key" | "nine_key_shuangpin"
  >(() => {
    const saved = readOptionalStorage("msime.keyboard.layout");
    return saved === "nine_key" || saved === "twenty_six_key" || saved === "nine_key_shuangpin"
      ? saved
      : layout;
  });
  function switchLayout() {
    const next =
      activeLayout === "twenty_six_key"
        ? layout === "nine_key_shuangpin"
          ? "nine_key_shuangpin"
          : "nine_key"
        : "twenty_six_key";
    setActiveLayout(next);
    writeOptionalStorage("msime.keyboard.layout", next);
  }
  const previousHostLayout = useRef(layout);
  useEffect(() => {
    // Initial mounting (including StrictMode replay) must preserve the saved preference.
    if (previousHostLayout.current === layout) return;
    previousHostLayout.current = layout;
    setActiveLayout(layout);
  }, [layout]);
  const sourceRows =
    activeLayout === "nine_key_shuangpin"
      ? nineKeyShuangpinRows(nineKeyShuangpinKeys ?? [])
      : activeLayout === "nine_key"
        ? nineKeyRows
        : keyboardRows;
  const rows =
    platform === "macos"
      ? sourceRows.map((row) => {
          // PC 底行左侧 Ctrl/Win/Alt 改为 Control/Option/Command，右侧按相反方向镜像。
          const ordered =
            row[0]?.virtualKey === 0x11 && row[3]?.virtualKey === 0x20
              ? [row[0], row[2], row[1], row[3], row[5], row[4], ...row.slice(6)]
              : row;
          return (
            ordered
              // macOS 没有这些 PC 专用键；保留有稳定 ANSI 映射的数字小键盘和导航键。
              .filter((item) => ![0x2c, 0x91, 0x13, 0x2d, 0x5d].includes(item.virtualKey))
              .map((item) => ({
                ...item,
                label:
                  item.label === "Win"
                    ? "Command"
                    : item.label === "Alt"
                      ? "Option"
                      : item.label === "Num Lock"
                        ? "Clear"
                        : item.label,
              }))
          );
        })
      : sourceRows;
  const [, setActiveModifiers] = useState<Set<Modifier>>(new Set());
  const modifiersRef = useRef<Set<Modifier>>(new Set());
  const [notice, setNotice] = useState(
    platform === "macos" ? MACOS_KEYBOARD_GUIDANCE : "Touch keyboard",
  );
  const platformRef = useLatestRef(platform);
  useEffect(() => {
    if (platform === "macos")
      setNotice((shown) => (shown === "Touch keyboard" ? MACOS_KEYBOARD_GUIDANCE : shown));
  }, [platform]);
  const [openingVoice, setOpeningVoice] = useState(false);
  type QueuedKey = { request: KeyboardInputRequest; description: string };
  const inputQueue = useRef<{
    active: boolean;
    running: boolean;
    openingVoice: boolean;
    pending: QueuedKey[];
  }>({ active: true, running: false, openingVoice: false, pending: [] });
  const keyRepeat = useRef<{ delay?: number; interval?: number }>({});
  // Windows sends on release, like the shipped KeyboardPanel::OnMouseUp: the key pressed on pointerdown is remembered and only sent if the pointer is released over that same key, so sliding off a key cancels it and there is no held-key repeat.
  const releaseKey = useRef<{ button: HTMLButtonElement; key: KeyboardKey } | null>(null);
  const sendsOnRelease = platform === "windows";
  const drag = usePanelDrag(client, () => setNotice("无法移动窗口，请重试。"));
  function stopKeyRepeat() {
    if (keyRepeat.current.delay !== undefined) window.clearTimeout(keyRepeat.current.delay);
    if (keyRepeat.current.interval !== undefined) window.clearInterval(keyRepeat.current.interval);
    keyRepeat.current = {};
  }
  useEffect(() => {
    const queue = { active: true, running: false, openingVoice: false, pending: [] as QueuedKey[] };
    setOpeningVoice(false);
    inputQueue.current = queue;
    modifiersRef.current = new Set();
    setActiveModifiers(new Set());
    if (client.rememberInputTarget)
      void client.rememberInputTarget().catch((error: unknown) => {
        if (queue.active) setNotice(keyboardDeliveryNotice(error, platformRef.current));
      });
    const stopBlur = subscribeToWindowBlur(stopKeyRepeat);
    return () => {
      stopBlur();
      stopKeyRepeat();
      queue.active = false;
      queue.pending = [];
    };
  }, [client]);
  function toggleModifier(keyToToggle: Modifier) {
    const next = new Set(modifiersRef.current);
    if (next.has(keyToToggle)) next.delete(keyToToggle);
    else next.add(keyToToggle);
    modifiersRef.current = next;
    setActiveModifiers(next);
    syncKeyboardFaces(next);
  }
  function syncKeyboardFaces(modifiers: Set<Modifier>) {
    // React may batch the Shift click with the following key click in a
    // synthetic test/event turn. Update the visible faces eagerly so the
    // keyboard remains truthful between those two events.
    if (typeof document === "undefined") return;
    const buttons = document.querySelectorAll<HTMLButtonElement>(
      "[data-keyboard-layout-grid] .keyboard-key",
    );
    rows.flat().forEach((item, index) => {
      const button = buttons[index];
      if (!button) return;
      const letter = item.label.length === 1 && /[a-z]/i.test(item.label);
      const shifted = modifiers.has("Shift") && item.label.length === 1;
      const face = shifted
        ? item.shifted || (letter ? item.label.toUpperCase() : item.label)
        : item.label;
      const label = button.querySelector<HTMLElement>("[data-key-label]");
      if (label) label.textContent = face;
      else button.textContent = face;
      if (item.modifier)
        button.setAttribute("aria-pressed", modifiers.has(item.modifier) ? "true" : "false");
    });
  }
  async function drainKeys() {
    const queue = inputQueue.current;
    if (!queue.active || queue.running || !client.sendKey) return;
    queue.running = true;
    try {
      while (queue.active && queue.pending.length) {
        const next = queue.pending.shift()!;
        setNotice(`正在发送：${next.description}`);
        try {
          // The Windows panel records the external foreground target on every
          // click because the editor may change while the non-activating panel
          // remains open. Keep that capture in the same serialized operation
          // as the key so a later click cannot overwrite the target before an
          // earlier key is delivered.
          if (client.rememberInputTarget) await client.rememberInputTarget();
          await client.sendKey(next.request);
        } catch (error: unknown) {
          // 发送可能已部分成功；不重放失败按键，清空队列并停止长按连发。
          queue.pending = [];
          stopKeyRepeat();
          if (queue.active) setNotice(keyboardDeliveryNotice(error, platform));
          return;
        }
        if (queue.active) setNotice(`已发送：${next.description}`);
      }
    } finally {
      queue.running = false;
    }
  }
  async function openVoiceKeyboard() {
    const queue = inputQueue.current;
    if (!queue.active || queue.openingVoice || !client.openVoice) return;
    if (queue.running || queue.pending.length) {
      setNotice("请等待按键发送完成后打开语音输入");
      return;
    }
    queue.openingVoice = true;
    setOpeningVoice(true);
    setNotice("正在打开语音输入…");
    try {
      await client.openVoice();
      if (queue.active) setNotice("已打开语音输入");
    } catch {
      if (queue.active) setNotice("无法打开语音输入，请重试");
    } finally {
      queue.openingVoice = false;
      if (queue.active && inputQueue.current === queue) setOpeningVoice(false);
    }
  }
  function closeKeyboard() {
    const queue = inputQueue.current;
    queue.active = false;
    queue.pending = [];
    stopKeyRepeat();
    void client.close().catch(() => {
      if (inputQueue.current === queue) {
        queue.active = true;
        setNotice("无法关闭键盘，请重试");
      }
    });
  }
  function pressKey(keyToPress: KeyboardKey) {
    const queue = inputQueue.current;
    if (!queue.active || queue.openingVoice) return;
    if (keyToPress.modifier) {
      toggleModifier(keyToPress.modifier);
      return;
    }
    if (queue.pending.length >= 64) {
      setNotice("按键正在发送，请稍候再继续输入");
      return;
    }
    const activeModifiers = modifiersRef.current;
    const shift = activeModifiers.has("Shift");
    const caps = activeModifiers.has("Caps Lock");
    const letter = keyToPress.label.length === 1 && /[a-z]/i.test(keyToPress.label);
    // Caps Lock and Shift invert one another for letters, just like the
    // Windows keyboard panel. Punctuation still follows Shift alone.
    const withShift = letter ? caps !== shift : shift;
    const modifiers = {
      ctrl: activeModifiers.has("Ctrl"),
      alt: activeModifiers.has("Alt"),
      win: activeModifiers.has("Win"),
    };
    const includeStickyModifiers = !isImeCommitKey(keyToPress.virtualKey);
    const prefix = modifierPrefix(activeModifiers);
    const displayedLabel = withShift
      ? keyToPress.shifted || (letter ? keyToPress.label.toUpperCase() : keyToPress.label)
      : keyToPress.label;
    const description = `${prefix}${prefix ? "+" : ""}${displayedLabel}`;
    const request: KeyboardInputRequest = {
      virtual_key: keyToPress.virtualKey,
      shift: withShift && includeStickyModifiers,
      modifiers,
      include_sticky_modifiers: includeStickyModifiers,
    };
    setNotice(
      client.sendKey ? `正在发送：${description}` : `已准备：${description}（等待宿主注入能力）`,
    );
    if (client.sendKey) {
      // The native reference sends each complete SendInput sequence
      // synchronously. Tauri commands are asynchronous, so preserve that
      // ordering explicitly instead of allowing later taps to overtake an
      // in-flight key or escape the bounded failure queue.
      queue.pending.push({ request, description });
      void drainKeys();
    }
    if (shift) {
      const next = new Set(activeModifiers);
      next.delete("Shift");
      modifiersRef.current = next;
      setActiveModifiers(next);
      syncKeyboardFaces(next);
    }
  }
  function endPointerKey(event: PointerEvent<HTMLButtonElement>) {
    stopKeyRepeat();
    const pressed = releaseKey.current;
    releaseKey.current = null;
    if (!sendsOnRelease || !pressed || pressed.button !== event.currentTarget) return;
    if (!event.isPrimary || event.button !== 0) return;
    pressKey(resolveRenderedKey(pressed.key, event.currentTarget.textContent ?? ""));
  }
  function cancelPointerKey() {
    stopKeyRepeat();
    releaseKey.current = null;
  }
  function beginPointerKey(event: PointerEvent<HTMLButtonElement>, keyToPress: KeyboardKey) {
    releaseKey.current = null;
    // Num Lock is a lock key in the Linux extended layout, so a held pointer
    // must not toggle it repeatedly like an ordinary keypad key.
    if (
      !event.isPrimary ||
      event.button !== 0 ||
      keyToPress.modifier ||
      keyToPress.virtualKey === 0x90 ||
      openingVoice
    )
      return;
    stopKeyRepeat();
    if (sendsOnRelease) {
      // Touch and pen capture the pointer implicitly, which would deliver the release to this key wherever it happens; release the capture so pointerup lands on the key actually under the pointer.
      if (event.currentTarget.hasPointerCapture?.(event.pointerId))
        event.currentTarget.releasePointerCapture(event.pointerId);
      releaseKey.current = { button: event.currentTarget, key: keyToPress };
      return;
    }
    pressKey(resolveRenderedKey(keyToPress, event.currentTarget.textContent ?? ""));
    keyRepeat.current.delay = window.setTimeout(() => {
      pressKey(keyToPress);
      keyRepeat.current.interval = window.setInterval(() => pressKey(keyToPress), 75);
    }, 450);
  }
  function resolveRenderedKey(fallback: KeyboardKey, displayed: string) {
    if (!modifiersRef.current.has("Shift")) return fallback;
    const match = rows
      .flat()
      .find(
        (item) =>
          item.shifted === displayed ||
          (item.shifted == null &&
            item.label.length === 1 &&
            item.label.toUpperCase() === displayed),
      );
    return match ?? fallback;
  }
  const { keySpacing: keyGap, rowSpacing: rowGap } = touchKeyboardSpacing(
    keySpacingTenths,
    rowSpacingTenths,
  );
  const keyboardStyle = {
    "--keyboard-key-gap": `${keyGap}px`,
    "--keyboard-row-gap": `${rowGap}px`,
  } as CSSProperties;
  const skinStyle = keyboardSkinStyles(theme, skin, customDesign);
  const renderedModifiers = modifiersRef.current;
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
          {activeLayout === "twenty_six_key" ? "九宫格" : "全键"}
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
              {row.map((keyToRender, keyIndex) => {
                const letter = keyToRender.label.length === 1 && /[a-z]/i.test(keyToRender.label);
                const shifted = renderedModifiers.has("Shift") && keyToRender.label.length === 1;
                const label = shifted
                  ? keyToRender.shifted ||
                    (letter ? keyToRender.label.toUpperCase() : keyToRender.label)
                  : keyToRender.label;
                return (
                  <button
                    type="button"
                    disabled={openingVoice}
                    key={`${keyToRender.label}-${keyIndex}`}
                    style={{
                      flexGrow:
                        activeLayout === "nine_key" || activeLayout === "nine_key_shuangpin"
                          ? 1
                          : keyboardKeyWeight(
                              keyToRender.label,
                              keyIndex,
                              row.some((item) => item.virtualKey === 0x20),
                            ),
                    }}
                    aria-pressed={
                      keyToRender.modifier ? renderedModifiers.has(keyToRender.modifier) : undefined
                    }
                    data-key-material={keyMaterial}
                    className={`keyboard-key ${surface.keyboardKey({
                      material: keyMaterial,
                      active: Boolean(
                        keyToRender.modifier && renderedModifiers.has(keyToRender.modifier),
                      ),
                      wide: keyToRender.label.length > 1,
                    })}`}
                    onPointerDown={(event) => beginPointerKey(event, keyToRender)}
                    onPointerUp={endPointerKey}
                    onPointerCancel={cancelPointerKey}
                    onPointerLeave={stopKeyRepeat}
                    onClick={(event) => {
                      // Pointer activation is delivered on pointerdown for immediate response and repeat, or on Windows on release over the same key. A detail-zero click comes from keyboard or assistive activation and still sends exactly one key.
                      if (
                        keyToRender.modifier ||
                        keyToRender.virtualKey === 0x90 ||
                        event.detail === 0
                      )
                        pressKey(
                          resolveRenderedKey(keyToRender, event.currentTarget.textContent ?? ""),
                        );
                    }}
                  >
                    <span data-key-label="">{label}</span>
                    {keyToRender.hint && (
                      <span className={surface.keyboardKeyHint}>{keyToRender.hint}</span>
                    )}
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </main>
  );
}

function HandwritingCandidateButton({
  candidate,
  copy,
  disabled,
  onChoose,
  onCopy,
}: {
  candidate: string;
  copy: boolean;
  disabled: boolean;
  onChoose: () => void;
  onCopy?: () => void;
}) {
  const button = useRef<HTMLButtonElement>(null);
  const [width, setWidth] = useState(64);
  useEffect(() => {
    const element = button.current;
    if (!element) return;
    const resize = () => setWidth(element.getBoundingClientRect().width);
    resize();
    return subscribeToElementResize(element, resize) ?? subscribeToWindowEvent("resize", resize);
  }, []);
  // Use Unicode code points so supplementary Han does not count as two glyphs.
  const length = Math.max(1, Array.from(candidate).length);
  const fontSize = clamp(Math.min(width * 0.52, (width - 12) / length), 13, 34);
  function copyShortcut(event: import("react").KeyboardEvent<HTMLButtonElement>) {
    if (
      !onCopy ||
      disabled ||
      event.defaultPrevented ||
      isComposingKeyEvent(event) ||
      !event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      event.shiftKey ||
      event.key.toLowerCase() !== "c"
    )
      return;
    event.preventDefault();
    if (!event.repeat) onCopy();
  }
  return (
    <button
      ref={button}
      type="button"
      className={surface.candidateSubmit}
      style={{ fontSize }}
      title={`${copy ? "复制" : "输入"}：${candidate}`}
      aria-keyshortcuts={onCopy ? "Control+c" : undefined}
      disabled={disabled}
      onClick={onChoose}
      onKeyDown={copyShortcut}
    >
      {candidate}
    </button>
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
        <section className={surface.inkSection}>
          <svg
            className={surface.inkCanvas}
            data-drawing={drawing.length > 0}
            tabIndex={0}
            viewBox="0 0 420 420"
            onPointerDown={start}
            onPointerMove={move}
            onPointerUp={end}
            onPointerCancel={cancel}
            onLostPointerCapture={cancel}
            aria-label="手写画布"
            aria-disabled={closing}
          >
            <g className={surface.inkGuides} aria-hidden="true">
              <path d="M210 20V400 M20 210H400 M20 20L400 400 M400 20L20 400" />
            </g>
            {renderStrokes.map((stroke, index) =>
              stroke.points.length === 1 ? (
                <circle key={index} cx={stroke.points[0].x} cy={stroke.points[0].y} r={2} />
              ) : (
                <polyline
                  key={index}
                  points={stroke.points.map(({ x, y }) => `${x},${y}`).join(" ")}
                />
              ),
            )}
            {!renderStrokes.length && (
              <text x="210" y="215" textAnchor="middle">
                请在这里书写
              </text>
            )}
          </svg>
          <div className={surface.inkActions}>
            <button
              type="button"
              className={surface.inkAction}
              onClick={undo}
              disabled={closing || (!strokes.length && !drawing.length)}
              aria-keyshortcuts="Control+z Meta+z"
            >
              ↶ 撤销
            </button>
            <button
              type="button"
              className={surface.inkAction}
              onClick={redo}
              disabled={closing || !redoStrokes.length || drawing.length > 0}
              aria-keyshortcuts="Control+Shift+z Meta+Shift+z Control+y"
            >
              ↷ 重做
            </button>
            <button
              type="button"
              className={surface.inkAction}
              onClick={clear}
              disabled={closing || (!strokes.length && !drawing.length && !redoStrokes.length)}
            >
              × 重写
            </button>
            {client.recognizeHandwriting && (
              <button
                type="button"
                className={surface.inkAction}
                onClick={retryRecognition}
                disabled={
                  closing || !strokes.length || drawing.length > 0 || recognizing || submitting
                }
              >
                {recognizing ? "识别中…" : "重新识别"}
              </button>
            )}
          </div>
        </section>
        <section className={surface.recognitionSection}>
          <h2>识别结果</h2>
          <div className={surface.handwritingActions} role="group" aria-label="点击手写候选的操作">
            <span>点击候选：</span>
            <PanelActivationButtons
              mode={effectiveMode}
              onModeChange={selectActivation}
              disabled={closing || submitting}
              copyDisabled={!client.copyHandwritingCandidate}
              inputAvailable={Boolean(client.submitHandwritingCandidate)}
            />
          </div>
          <div
            className={surface.candidateGrid}
            // Named so the grid can be reached as a unit: assistive technology announces the group,
            // and a test has something to hold onto that is not a styling class.
            role="group"
            aria-label="识别候选"
            onKeyDown={navigateCandidates}
          >
            {candidates.map((candidate) => (
              <div className={surface.candidate} key={candidate}>
                <HandwritingCandidateButton
                  candidate={candidate}
                  copy={effectiveMode === "copy"}
                  disabled={closing || submitting}
                  onChoose={() => void chooseCandidate(candidate, effectiveMode === "copy")}
                  onCopy={
                    client.copyHandwritingCandidate
                      ? () => void chooseCandidate(candidate, true)
                      : undefined
                  }
                />
                {effectiveMode === "copy" && client.submitHandwritingCandidate ? (
                  <button
                    type="button"
                    className={surface.candidateCopy}
                    aria-label={`输入候选 ${candidate}`}
                    disabled={closing || submitting}
                    onClick={() => void chooseCandidate(candidate)}
                  >
                    输入
                  </button>
                ) : (
                  effectiveMode === "input" &&
                  client.copyHandwritingCandidate && (
                    <button
                      type="button"
                      className={surface.candidateCopy}
                      aria-label={`复制候选 ${candidate}`}
                      disabled={closing || submitting}
                      onClick={() => void chooseCandidate(candidate, true)}
                    >
                      复制
                    </button>
                  )
                )}
              </div>
            ))}
          </div>
          {!candidates.length && (
            <div className={surface.handwritingEmpty} aria-hidden="true">
              <span>
                <FluentIcon name="pen" size={26} />
              </span>
              <p>写下一个字，候选会出现在这里</p>
              <small>每写完一笔，自动更新识别结果</small>
            </div>
          )}
          {candidates.length > 0 && <div className="flex-1" />}
          {handwritingPack && <HandwritingModelNotice packs={packs} mirror={client.modelMirror} />}
          <StatusMessage role="status">{notice}</StatusMessage>
        </section>
      </div>
    </main>
  );
}

/** 手写模型下载中的进度与取消、下载失败的原因与重试（宿主能读写镜像时还有设置下载镜像的入口），或取消后仍缺模型时的下载入口；模型已在、也没有下载时不渲染。 */
function HandwritingModelNotice({
  packs,
  mirror,
}: {
  packs: ResourcePacks;
  mirror?: ModelMirrorClient;
}) {
  const status = resourcePackStatus(packs, "handwriting");
  const progress = packs.progress.handwriting;
  const error = packs.errors.handwriting;
  if (progress) {
    return (
      <p className={surface.handwritingActions} aria-live="polite">
        <span>
          正在下载手写模型（约 {formatModelBytes(status?.size ?? progress.total)}）…{" "}
          {localModelProgressPercent(progress)}%
        </span>
        <button type="button" onClick={() => packs.cancel("handwriting")}>
          取消
        </button>
      </p>
    );
  }
  if (error) {
    return (
      <>
        <p className={surface.handwritingActions} aria-live="polite">
          <span>{error}</span>
          <button type="button" onClick={() => packs.install("handwriting")}>
            重试
          </button>
        </p>
        {mirror && (
          <HandwritingMirrorEditor mirror={mirror} onSaved={() => packs.install("handwriting")} />
        )}
      </>
    );
  }
  if (status?.state === "missing") {
    return (
      <p className={surface.handwritingActions} aria-live="polite">
        <span>手写识别需要先下载手写模型（约 {formatModelBytes(status.size)}）</span>
        <button type="button" onClick={() => packs.install("handwriting")}>
          下载
        </button>
      </p>
    );
  }
  return null;
}

/** 手写模型下载失败后的镜像设置：先是一个「设置下载镜像」按钮，点开后读出已保存的镜像、就地编辑，「保存并重试」写入偏好后重新下载。 */
function HandwritingMirrorEditor({
  mirror,
  onSaved,
}: {
  mirror: ModelMirrorClient;
  onSaved: () => void;
}) {
  const [value, setValue] = useState<string>();
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  const mounted = useMountedRef();
  async function open() {
    try {
      const saved = await mirror.load();
      if (mounted.current) setValue(saved);
    } catch {
      if (mounted.current) setMessage("无法读取下载镜像，请重试。");
    }
  }
  async function save() {
    if (value === undefined || saving) return;
    const next = value.trim();
    if (!validModelMirror(next)) {
      setMessage("下载镜像地址无效，必须以 https:// 开头。");
      return;
    }
    setSaving(true);
    setMessage("");
    try {
      await mirror.save(next);
      if (!mounted.current) return;
      onSaved();
    } catch {
      if (mounted.current) setMessage("无法保存下载镜像，请重试。");
    } finally {
      if (mounted.current) setSaving(false);
    }
  }
  if (value === undefined) {
    return (
      <p className={surface.handwritingActions}>
        {message && <span>{message}</span>}
        <button type="button" onClick={() => void open()}>
          设置下载镜像
        </button>
      </p>
    );
  }
  return (
    <p className={surface.handwritingActions}>
      <input
        aria-label="模型下载镜像"
        placeholder="https://mirror.example.com"
        maxLength={2048}
        value={value}
        aria-invalid={!validModelMirror(value.trim())}
        disabled={saving}
        onChange={(event) => setValue(event.target.value)}
      />
      <button type="button" disabled={saving} onClick={() => void save()}>
        保存并重试
      </button>
      {message && <span>{message}</span>}
    </p>
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
        <textarea
          className={surface.voiceTextArea}
          aria-label="识别结果"
          aria-describedby={exceedsSubmitLimit ? "voice-result-limit" : undefined}
          aria-invalid={exceedsSubmitLimit || undefined}
          value={text}
          maxLength={4096}
          onChange={(event) => updateText(event.target.value)}
          placeholder="识别结果会显示在这里"
          rows={4}
        />
        {exceedsSubmitLimit && (
          <p id="voice-result-limit" className={surface.voiceNote} role="status">
            内容超过单次提交长度，请精简或复制结果后手动粘贴。
          </p>
        )}
        <button
          type="button"
          className={surface.voiceSubmit}
          onClick={() => void submit()}
          disabled={
            !text ||
            exceedsSubmitLimit ||
            !(client.sendVoiceText ?? client.sendText) ||
            busy ||
            submitting
          }
        >
          {submitting && !copying ? "正在提交…" : "提交到当前窗口"}
        </button>
        {client.copyText && (
          <button
            type="button"
            onClick={() => void submit(true)}
            disabled={!text || busy || submitting}
          >
            {copying ? "正在复制…" : "复制结果"}
          </button>
        )}
        <button type="button" onClick={clear} disabled={!text || busy}>
          清空结果
        </button>
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
            <div className={cloud.clipboardAddRow}>
              <textarea
                className={cloud.clipboardTextArea}
                aria-label="待上传文本"
                value={draft}
                onChange={(event) => {
                  updateDraft(event.target.value);
                }}
                placeholder="输入要上传的文本"
                rows={3}
              />
              <small className={cloud.clipboardCount(draft.length > 4000)}>
                {draft.length} / 4000
              </small>
              <button
                type="button"
                className={cloud.clipboardSubmit}
                onClick={() => void add()}
                disabled={!enabled || draft.trim().length === 0 || draft.length > 4000 || busy}
              >
                上传明确选择的文本
              </button>
            </div>
            {e2e && e2e.undecryptable > 0 && (
              <p className={cloud.clipboardNote} role="note">
                有 {e2e.undecryptable}{" "}
                条云端记录无法在本设备解密，已略去。它们可能是用另一把密钥写入的。
              </p>
            )}
            <div className={cloud.clipboardList} aria-label="云端历史">
              {items.length ? (
                items.map((item) => (
                  <article className={cloud.clipboardItem} key={item.id}>
                    <button type="button" disabled={busy} onClick={() => void choose(item)}>
                      {item.text}
                    </button>
                    {client.copyText && (
                      <button
                        type="button"
                        disabled={busy}
                        aria-label="复制此条云端记录"
                        onClick={() => void choose(item, true)}
                      >
                        复制
                      </button>
                    )}
                    <button
                      type="button"
                      className={cloud.clipboardDelete}
                      aria-label={`删除 ${item.text}`}
                      onClick={() => void remove(item.id)}
                      disabled={busy}
                    >
                      删除
                    </button>
                  </article>
                ))
              ) : (
                <p className={cloud.clipboardNote}>暂无云端历史</p>
              )}
            </div>
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
        <section className={cloud.dictionarySection} aria-label="准备本机词库">
          <h2>准备本机词库</h2>
          <p className={cloud.dictionaryNote}>
            请启用水杉键盘并打开一次，让宿主提供当前本机词库版本。测试区或编辑器内容不会上传。
          </p>
          <div className={cloud.dictionarySnapshotFact}>
            {localVersion ? (
              <>
                <strong>已获取本机词库版本</strong>
                <small>{localVersion}</small>
              </>
            ) : (
              <strong>尚未获取本机词库版本</strong>
            )}
          </div>
          <button
            className={cloud.dictionaryButton}
            type="button"
            onClick={download}
            disabled={
              busy ||
              !localVersion ||
              request?.status === "queued" ||
              request?.status === "preparing"
            }
          >
            下载云词库并预览
          </button>
          <button
            type="button"
            className="secondary"
            onClick={() => void refreshStatus()}
            disabled={busy}
          >
            刷新状态
          </button>
        </section>
        {preview && (
          <section className={cloud.dictionarySection} aria-label="确认应用">
            <h2>确认应用</h2>
            <p className={cloud.dictionaryNote}>
              云端 revision {preview.cloudRevision} · {preview.records} 条记录 · {preview.bytes}{" "}
              字节
            </p>
            <p className={cloud.dictionaryNote}>
              词条 {preview.entries} · 覆盖 {preview.overlays} · 固定 {preview.positions} · 选择{" "}
              {preview.selections}
            </p>
            <button type="button" onClick={enqueue} disabled={busy || !previewToken}>
              替换本机词库
            </button>
            <button type="button" className="secondary" onClick={discardPreview} disabled={busy}>
              丢弃预览
            </button>
          </section>
        )}
        {request && (
          <section className={cloud.dictionarySection} aria-label="处理结果">
            <h2>处理结果</h2>
            <p className={cloud.dictionaryNote}>
              状态：{request.status} · 云端 revision {request.cloudRevision}
            </p>
            {client.snapshotAppliesInPanel &&
              (request.status === "queued" || request.status === "preparing") && (
                <p className={cloud.dictionaryNote}>
                  快照只在这个面板开着时应用，关掉面板会停在待应用；之后输入改变了本机词库，这份快照会因冲突作废。
                </p>
              )}
            {(request.status === "queued" || request.status === "preparing") && (
              <button type="button" className="secondary" onClick={cancel} disabled={busy}>
                取消待应用快照
              </button>
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
          <div className={cloud.dictionaryList} aria-label="云端候选结果">
            <p>
              查询编码：{query.text} · 云端版本 {revision}
            </p>
            {candidates.length ? (
              candidates.map((candidate, index) => (
                <CloudDictionaryItem
                  key={`${candidateMutationCode(candidate)}:${candidate.word}`}
                  ariaLabel={`调频候选 ${candidate.word}`}
                  busy={busy}
                  disabled={kind === "quick"}
                  onClick={() => void rank(candidate)}
                  actions={
                    kind !== "quick" && (
                      <>
                        <button
                          type="button"
                          className="secondary"
                          onClick={() => void rank(candidate)}
                          disabled={busy}
                        >
                          调频
                        </button>
                        <button
                          type="button"
                          className="secondary"
                          onClick={() => void setFixed(candidate, position)}
                          disabled={busy}
                        >
                          固定
                        </button>
                        <button
                          type="button"
                          className="secondary"
                          onClick={() => void remove(candidate)}
                          disabled={
                            busy || (kind !== "english" && Array.from(candidate.word).length <= 1)
                          }
                        >
                          删除
                        </button>
                      </>
                    )
                  }
                >
                  <strong>
                    {index + 1}. <span className="wrap-anywhere">{candidate.word}</span>
                  </strong>
                  <small>
                    {candidate.code} · 权重 {candidate.weight}
                  </small>
                </CloudDictionaryItem>
              ))
            ) : (
              <p className={cloud.dictionaryEmpty}>没有匹配的候选</p>
            )}
          </div>
        )}
        {query && candidates.length > 0 && (
          <p className={cloud.dictionaryMobileHint}>
            点按候选可调频；窄屏下的固定和删除操作会分组显示。
          </p>
        )}
        {positions.length > 0 && (
          <div className={cloud.dictionaryList} aria-label="固定位置">
            <p>此查询的固定位置</p>
            {positions.map((item) => (
              <article
                className={cloud.dictionaryItem}
                key={`${item.context}:${item.code}:${item.word}`}
              >
                <div className={`${cloud.dictionaryItemMain} ${cloud.dictionaryItemStatic}`}>
                  <strong>
                    第 {item.position} 位 · {item.word}
                  </strong>
                  <small>{item.code}</small>
                </div>
                <div className={cloud.dictionaryItemActions}>
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => void unfix(item)}
                    disabled={busy}
                  >
                    取消固定
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
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
/** A raised field: the search box, a clipboard row, the delete button beside one. */
const panelField =
  "border border-[#3a3a44] bg-[#2b2b33] text-[#f5f5f7] group-data-[panel-theme=light]:border-[#c8ccd5] group-data-[panel-theme=light]:bg-white group-data-[panel-theme=light]:text-[#202124]";
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
/*
 * A tile. Grid tiles are uniform squares; flow tiles hold kaomoji and captioned symbols, so they size
 * to their content and wrap their text instead. The `:nth-child(n)` the stylesheet used on the group
 * was only there to win a specificity fight with the grid rule -- a fight utilities do not have.
 */
const panelTile = (flow: boolean, compact = false) =>
  `flex min-w-0 items-center justify-center overflow-hidden rounded-[9px] border border-transparent bg-transparent text-center font-emoji-text leading-[1.2] wrap-anywhere ${surface.panelText} hover:border-[#555560] hover:bg-[#303038] focus-visible:border-[#555560] focus-visible:bg-[#303038] group-data-[panel-theme=light]:hover:border-[#c8ccd5] group-data-[panel-theme=light]:hover:bg-[#eceef3] group-data-[panel-theme=light]:focus-visible:border-[#c8ccd5] group-data-[panel-theme=light]:focus-visible:bg-[#eceef3] disabled:cursor-wait disabled:opacity-55 ${
    flow
      ? "max-w-full flex-[0_1_auto] min-h-12 px-3 py-2.5 text-xl whitespace-pre-wrap"
      : compact
        ? "h-10 min-h-10 p-1 text-xl"
        : "min-h-[66px] p-1.5 text-[29px]"
  }`;
const clipboardRow = `w-full flex-1 min-w-0 overflow-hidden rounded-lg px-3.5 py-3 text-left font-[inherit] text-ellipsis whitespace-nowrap ${panelField} hover:border-[#5fbf84] hover:bg-[#303038] focus-visible:border-[#5fbf84] focus-visible:bg-[#303038] group-data-[panel-theme=light]:hover:border-[#2c7a4b] group-data-[panel-theme=light]:hover:bg-[#eceef3] group-data-[panel-theme=light]:focus-visible:border-[#2c7a4b] group-data-[panel-theme=light]:focus-visible:bg-[#eceef3]`;
const clipboardSideButton = `shrink-0 grow-0 basis-auto rounded-lg p-2 font-[inherit] ${panelField} hover:border-[#5fbf84] focus-visible:border-[#5fbf84] group-data-[panel-theme=light]:hover:border-[#2c7a4b] group-data-[panel-theme=light]:focus-visible:border-[#2c7a4b]`;

export function EmojiPanel({
  client,
  theme = "dark",
  initialPage = "home",
}: {
  client: EmojiPanelClient;
  theme?: "dark" | "light";
  initialPage?: "home" | "clipboard";
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
  } = useEmojiPanel(client, initialPage);
  return (
    <main
      {...navigation}
      className={`native-panel group flex min-h-screen flex-col overflow-hidden ${page === "symbols" ? "h-screen" : ""} ${surface.panelSurface} ${surface.panelText}`}
      data-panel-theme={theme}
      aria-label="表情与符号"
    >
      <NativePanelHeader
        title={page === "symbols" ? "符号大全" : "Emoji and more"}
        className={`native-panel-header flex-[0_0_38px] border-b ${panelDivider} ${surface.panelSurface} ${surface.panelText} [&>button]:text-[#aeb0b7] group-data-[panel-theme=light]:[&>button]:text-[#656a73]`}
        closeDisabled={clipboardBusy}
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
      {isDetail && page !== "symbols" && (
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
                  <button
                    type="button"
                    key={item.text}
                    className={`${panelTile(false, true)} shrink-0 min-w-10`}
                    title={emojiDisplayName(item.keywords, item.text)}
                    onClick={() => copy(item.text, false, item.keywords)}
                  >
                    {item.text}
                  </button>
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
                  {tab.id.startsWith("pack") ? tab.title : symbolCategoryTitle(tab.title)}
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
                      <button
                        type="button"
                        key={`${index}-${item.text}`}
                        className={panelTile(false, true)}
                        data-emoji-navigation-item
                        title={emojiDisplayName(item.keywords, item.text)}
                        onClick={() => copy(item.text, false, item.keywords)}
                      >
                        {item.text}
                      </button>
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
            <div className="flex flex-col gap-[7px]">
              {visibleClipboard.map((item) => (
                <div className="flex items-stretch gap-1.5" data-clipboard-row="" key={item}>
                  <button
                    type="button"
                    className={clipboardRow}
                    data-clipboard-item=""
                    data-emoji-navigation-item
                    aria-keyshortcuts={
                      [canCopy ? "Control+c" : "", client.clipboard?.remove ? "Delete" : ""]
                        .filter(Boolean)
                        .join(" ") || undefined
                    }
                    onKeyDown={(event) => clipboardItemKey(event, item)}
                    disabled={clipboardBusy}
                    title={clipboardTooltip(item)}
                    onClick={() => void copy(item, true)}
                  >
                    {item.replace(/[\r\n\t]/g, " ")}
                  </button>
                  {client.clipboard?.paste && (
                    <button
                      type="button"
                      className={clipboardSideButton}
                      disabled={clipboardBusy}
                      aria-label="粘贴此条记录到原应用"
                      onClick={() => void pasteClipboard(item)}
                    >
                      粘贴
                    </button>
                  )}
                  {client.clipboard?.remove && (
                    <button
                      type="button"
                      className={clipboardSideButton}
                      disabled={clipboardBusy}
                      aria-label="删除此条记录"
                      onClick={() => void removeClipboard(item)}
                    >
                      删除
                    </button>
                  )}
                </div>
              ))}
            </div>
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
                  <button
                    type="button"
                    className={panelTile(group.flow === true)}
                    data-emoji-navigation-item
                    disabled={clipboardBusy}
                    key={`${index}-${item.text}`}
                    title={emojiDisplayName(item.keywords, item.text)}
                    onClick={() => void copy(item.text, false, item.keywords)}
                  >
                    {item.text}
                  </button>
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
    </main>
  );
}
