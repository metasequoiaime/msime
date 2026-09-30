import { useConfirm } from "../core/confirm";
import { clamp } from "../core/number";
import { utf8ByteLength } from "../core/text";
import { readDictionaryFile } from "../dictionary/dictionary-file";
import { usePanelDrag } from "./use-panel-drag";
import { useEmojiNavigation } from "../emoji/use-emoji-navigation";
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent,
} from "react";
import {
  fallbackEmojiGroups,
  fallbackKaomojiGroups,
  fallbackSymbolGroups,
  type EmojiCatalogGroup,
  type EmojiCatalogItem,
} from "../emoji/emoji-catalog";
import type { GlobalTheme } from "../theme/global-theme";
import * as cloud from "./cloud-panel-style";
import * as surface from "./panel-surface-style";
import { normalizeHandwritingCandidates } from "./handwriting";
import { validVoiceLanguage } from "./voice-panel";
import {
  isImeCommitKey,
  keyboardKeyWeight,
  modifierPrefix,
  type KeyboardKey,
  type Modifier,
} from "./keyboard-input";
import { keyboardSkinStyles } from "./keyboard-skin-styles";
import { keyboardRows, nineKeyRows } from "./panel-keyboard-layouts";
import { CloudDictionaryEntryForm } from "./cloud-dictionary-entry-form";
import { CloudPanelHeader } from "./cloud-panel-header";
import {
  clipboardTooltip,
  emojiDisplayName,
  flattenGroups,
  matchesEmojiItem,
} from "./emoji-panel-helpers";

export { emojiDisplayName } from "./emoji-panel-helpers";
import {
  appendPointerSamples,
  MAX_HANDWRITING_STROKES,
  type Point,
  WINDOWS_HANDWRITING_STROKES,
} from "./handwriting-input";
import {
  candidateMutationCode,
  cloudClipboardItems,
  cloudDictionaryCatalogEntries,
  cloudDictionaryEntries,
  cloudResponseRequest,
  cloudResponseText,
} from "./cloud-response";
import type { TouchKeyboardSkinDesign } from "./touch-keyboard-skin-design";

export interface KeyboardInputRequest {
  virtual_key: number;
  shift: boolean;
  modifiers: { ctrl: boolean; alt: boolean; win: boolean };
  include_sticky_modifiers: boolean;
}

export interface InkPoint {
  x: number;
  y: number;
}
export interface InkStroke {
  points: InkPoint[];
}
export interface HandwritingRecognitionRequest {
  language: string;
  strokes: InkStroke[];
}
export interface HandwritingRecognitionResult {
  candidates: string[];
}

export interface PanelClient {
  close(): Promise<void>;
  openVoice?(): Promise<void>;
  beginWindowDrag?(): Promise<void>;
  rememberInputTarget?(): Promise<void>;
  sendKey?(request: KeyboardInputRequest): Promise<void>;
  sendText?(text: string): Promise<void>;
  recognizeHandwriting?(
    request: HandwritingRecognitionRequest,
  ): Promise<HandwritingRecognitionResult>;
  submitHandwritingCandidate?(candidate: string): Promise<void>;
  copyHandwritingCandidate?(candidate: string): Promise<void>;
}

export interface VoicePanelClient extends PanelClient {
  maxSubmitBytes?: number;
  /** Platform-specific explanation shown above the recording controls. */
  description?: string;
  /** Optional handoff wording for hosts that submit to another native surface. */
  submitNotice?: string;
  loadVoiceLanguage?(): Promise<string>;
  recognizeVoice?(language: string): Promise<{ text: string }>;
  onVoiceUpdate?(
    listener: (update: {
      text: string;
      final: boolean;
      phase?: "recording" | "recognizing" | "polishing";
      level?: number;
    }) => void,
  ): Promise<() => void>;
  cancelVoice?(): Promise<void>;
  stopVoice?(): Promise<void>;
  sendVoiceText?(text: string): Promise<void>;
  copyText?(text: string): Promise<void>;
}

export type CloudClipboardAction =
  | { operation: "list"; search: string }
  | { operation: "add"; text: string }
  | { operation: "delete"; id: string }
  | { operation: "set_enabled"; enabled: boolean };
export type CloudClipboardItem = { id: string; text: string };
export interface CloudClipboardPanelClient extends PanelClient {
  canSendText?(): Promise<boolean>;
  copyText?(text: string): Promise<void>;
  request(
    action: CloudClipboardAction,
  ): Promise<{ items?: CloudClipboardItem[]; enabled?: boolean }>;
}

export type CloudDictionaryKind = "pinyin" | "wubi" | "quick" | "english";
export type CloudCandidateKind = CloudDictionaryKind | "jianpin";
export type CloudRankingMode = "disabled" | "pin" | "halve" | "linear" | "promote";
export type CloudDictionaryFileFormat = "standard" | "windows" | "hans";
export type CloudDictionaryAction =
  | { operation: "snapshot_preview" }
  | { operation: "snapshot_export" }
  | { operation: "snapshot_restore_preview"; text: string }
  | { operation: "snapshot_restore"; text: string; expected_sha256: string; revision: number }
  | { operation: "snapshot_restore_native"; token: string }
  | { operation: "snapshot_restore_cancel" }
  | { operation: "snapshot_enqueue"; token: string }
  | { operation: "snapshot_status" }
  | { operation: "snapshot_cancel" }
  | { operation: "list"; kind: CloudDictionaryKind; offset: number; search: string }
  | {
      operation: "catalog";
      kind: CloudDictionaryKind;
      code: string;
      offset: number;
      scheme: string;
      profile: string;
    }
  | { operation: "add"; kind: CloudDictionaryKind; code: string; word: string; weight: number }
  | {
      operation: "update";
      kind: CloudDictionaryKind;
      id: string;
      code: string;
      word: string;
      weight: number;
      revision: number;
    }
  | {
      operation: "edit_catalog";
      kind: CloudDictionaryKind;
      code: string;
      word: string;
      revision: number;
      replacement: { code: string; word: string; weight: number } | null;
    }
  | {
      operation: "candidates";
      text: string;
      kind: CloudCandidateKind;
      scheme: string;
      profile: string;
      limit: number;
    }
  | {
      operation: "rank";
      text: string;
      kind: CloudCandidateKind;
      scheme: string;
      profile: string;
      limit: number;
      code: string;
      word: string;
      revision: number;
      mode: CloudRankingMode;
      linear_step: number;
      trigger_count: number;
      force_top: boolean;
    }
  | {
      operation: "remove_candidate";
      text: string;
      kind: CloudCandidateKind;
      scheme: string;
      profile: string;
      limit: number;
      code: string;
      word: string;
      revision: number;
    }
  | { operation: "fixed_positions"; context: string; offset: number }
  | {
      operation: "set_fixed_position";
      context: string;
      code: string;
      word: string;
      position: number | null;
      revision: number;
    }
  | { operation: "delete"; kind: CloudDictionaryKind; id: string; revision: number }
  | {
      operation: "import";
      kind: CloudDictionaryKind;
      format: CloudDictionaryFileFormat;
      text: string;
    }
  | {
      operation: "export";
      kind: CloudDictionaryKind;
      format: Exclude<CloudDictionaryFileFormat, "hans">;
    };
export type CloudDictionaryEntry = {
  id: string;
  kind: CloudDictionaryKind;
  code: string;
  word: string;
  weight: number;
  revision: number;
};
export type CloudDictionaryCatalogEntry = {
  kind: CloudDictionaryKind;
  code: string;
  word: string;
  weight: number;
};
export type CloudCandidate = {
  code: string;
  word: string;
  weight: number;
  canonical_pinyin?: string | null;
};
export type CloudFixedPosition = { context: string; code: string; word: string; position: number };
export type CloudDictionarySnapshotMetadata = {
  cloudRevision: number;
  sha256: string;
  bytes: number;
  records: number;
  entries: number;
  overlays: number;
  positions: number;
  selections: number;
};
export type CloudDictionarySnapshotRequest = {
  id: string;
  cloudRevision: number;
  expectedLocalVersion?: string;
  fileSha256: string;
  status: "queued" | "preparing" | "applied" | "conflict" | "failed" | "cancelled";
};
export type CloudDictionaryResponse = {
  saved?: boolean;
  previewToken?: string;
  snapshot?: CloudDictionarySnapshotMetadata;
  expectedRevision?: number;
  localVersion?: string;
  request?: CloudDictionarySnapshotRequest;
  entries?: CloudDictionaryEntry[];
  catalog_entries?: CloudDictionaryCatalogEntry[];
  candidates?: CloudCandidate[];
  positions?: CloudFixedPosition[];
  has_more?: boolean;
  offset?: number;
  revision?: number;
  context?: string;
  normalized?: string;
  changed?: boolean;
  selection_count?: number;
  text?: string;
  content?: string;
  filename?: string;
};
export interface CloudDictionaryPanelClient extends PanelClient {
  request(action: CloudDictionaryAction): Promise<CloudDictionaryResponse>;
  snapshot?: boolean;
  snapshotNative?: boolean;
  exportNative?: boolean;
  chooseSnapshotRestore?(): Promise<CloudDictionaryResponse>;
  downloadToLocal?(entry: CloudDictionaryEntry): Promise<void>;
  openCatalog?(): Promise<void>;
  openCandidates?(): Promise<void>;
  openFiles?(): Promise<void>;
  openApply?(): Promise<void>;
  back?(): Promise<void>;
}

export interface EmojiPanelClient extends PanelClient {
  copyText?(text: string): Promise<void>;
  clipboard?: {
    list?(): Promise<string[]>;
    isEnabled?(): Promise<boolean>;
    enable?(): Promise<void>;
    onChanged?(listener: () => void): Promise<() => void>;
    sync?(): Promise<string[]>;
    remove?(text: string): Promise<void>;
    clear?(): Promise<void>;
    copy?(text: string): Promise<void>;
    paste?(text: string): Promise<void>;
  };
  loadCatalog?(): Promise<{
    emoji: EmojiCatalogGroup[];
    kaomoji: EmojiCatalogGroup[];
    symbols: EmojiCatalogGroup[];
    unavailable?: ("emoji" | "kaomoji" | "symbols")[];
  }>;
}

export function KeyboardPanel({
  client,
  platform,
  theme = "dark",
  layout = "twenty_six_key",
  keySpacingTenths = 60,
  rowSpacingTenths = 70,
  voiceShortcut = false,
  skin = "system",
  customDesign,
}: {
  client: PanelClient;
  platform?: string;
  theme?: "dark" | "light";
  layout?: "twenty_six_key" | "nine_key";
  keySpacingTenths?: number;
  rowSpacingTenths?: number;
  voiceShortcut?: boolean;
  skin?: GlobalTheme;
  customDesign?: TouchKeyboardSkinDesign;
}) {
  const [activeLayout, setActiveLayout] = useState<"twenty_six_key" | "nine_key">(() => {
    let saved: string | null = null;
    try {
      saved =
        typeof window !== "undefined" ? window.localStorage.getItem("msime.keyboard.layout") : null;
    } catch {
      /* restricted webviews may deny storage */
    }
    return saved === "nine_key" || saved === "twenty_six_key" ? saved : layout;
  });
  function switchLayout() {
    const next = activeLayout === "nine_key" ? "twenty_six_key" : "nine_key";
    setActiveLayout(next);
    try {
      window.localStorage.setItem("msime.keyboard.layout", next);
    } catch {
      /* preference is optional */
    }
  }
  const previousHostLayout = useRef(layout);
  useEffect(() => {
    // Initial mounting (including StrictMode replay) must preserve the saved preference.
    if (previousHostLayout.current === layout) return;
    previousHostLayout.current = layout;
    setActiveLayout(layout);
  }, [layout]);
  const sourceRows = activeLayout === "nine_key" ? nineKeyRows : keyboardRows;
  const rows =
    platform === "macos"
      ? sourceRows.map((row) =>
          row
            // macOS has no PC application/Menu key. The native panel omits it and
            // host-macos deliberately rejects the Windows VK_MENU (0x5d) contract
            // value rather than guessing at a Command/Option equivalent. Keep the
            // shared layout's other keypad/navigation keys because CoreGraphics has
            // stable ANSI mappings for those values.
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
            })),
        )
      : sourceRows;
  const [, setActiveModifiers] = useState<Set<Modifier>>(new Set());
  const modifiersRef = useRef<Set<Modifier>>(new Set());
  const [notice, setNotice] = useState("Touch keyboard");
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
      void client.rememberInputTarget().catch(() => {
        if (queue.active) setNotice("未能记录前台输入窗口");
      });
    const stopForBlur = () => stopKeyRepeat();
    window.addEventListener("blur", stopForBlur);
    return () => {
      window.removeEventListener("blur", stopForBlur);
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
      button.textContent = shifted
        ? item.shifted || (letter ? item.label.toUpperCase() : item.label)
        : item.label;
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
        } catch {
          // Delivery may have partially succeeded; never replay a failed key.
          queue.pending = [];
          stopKeyRepeat();
          if (queue.active) setNotice("按键发送失败，后续排队按键已取消，请确认输入位置后继续");
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
  const keyGap = clamp(keySpacingTenths / 10, 3, 6);
  const rowGap = clamp(rowSpacingTenths / 10, 4, 10);
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
      <header className={`native-panel-header ${surface.keyboardHeader}`} {...drag}>
        <span className={surface.keyboardNotice} role="status" title={notice}>
          {notice}
        </span>
        <button type="button" aria-label="切换键盘布局" onClick={switchLayout}>
          {activeLayout === "nine_key" ? "全键" : "九宫格"}
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
        <button type="button" aria-label="关闭" disabled={openingVoice} onClick={closeKeyboard}>
          ×
        </button>
      </header>
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
                        activeLayout === "nine_key"
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
                    {label}
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
    if (typeof ResizeObserver !== "undefined") {
      const observer = new ResizeObserver(resize);
      observer.observe(element);
      return () => observer.disconnect();
    }
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);
  // Use Unicode code points so supplementary Han does not count as two glyphs.
  const length = Math.max(1, Array.from(candidate).length);
  const fontSize = clamp(Math.min(width * 0.52, (width - 12) / length), 13, 34);
  function copyShortcut(event: import("react").KeyboardEvent<HTMLButtonElement>) {
    if (
      !onCopy ||
      disabled ||
      event.defaultPrevented ||
      event.nativeEvent.isComposing ||
      event.keyCode === 229 ||
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
  const maxStrokes = platform === "windows" ? WINDOWS_HANDWRITING_STROKES : MAX_HANDWRITING_STROKES;
  const [activationMode, setActivationMode] = useState<"copy" | "input">(() => {
    try {
      return window.localStorage.getItem("msime.handwriting.activation") === "input"
        ? "input"
        : "copy";
    } catch {
      return "copy";
    }
  });
  const effectiveMode =
    activationMode === "copy" && !client.copyHandwritingCandidate
      ? "input"
      : activationMode === "input" && !client.submitHandwritingCandidate
        ? "copy"
        : activationMode;
  function selectActivation(mode: "copy" | "input") {
    if (!recognitionQueue.current.active || closingRef.current) return;
    setActivationMode(mode);
    try {
      window.localStorage.setItem("msime.handwriting.activation", mode);
    } catch {
      /* optional preference */
    }
  }
  const [strokes, setStrokes] = useState<InkStroke[]>([]);
  const [redoStrokes, setRedoStrokes] = useState<InkStroke[]>([]);
  const [drawing, setDrawing] = useState<Point[]>([]);
  const [candidates, setCandidates] = useState<string[]>([]);
  const [notice, setNotice] = useState("请在左侧书写，松开鼠标后自动识别");
  const drag = usePanelDrag(client, () => setNotice("无法移动窗口，请重试。"));
  const recognitionRevision = useRef(0);
  const [recognizing, setRecognizing] = useState(false);
  const recognitionQueue = useRef<{
    active: boolean;
    running: boolean;
    pending: { revision: number; strokes: InkStroke[] } | null;
  }>({ active: true, running: false, pending: null });
  const submissionRevision = useRef(0);
  const closingRef = useRef(false);
  const [closing, setClosing] = useState(false);
  const submittingRef = useRef(false);
  const [submitting, setSubmitting] = useState(false);
  const activeStroke = useRef<{ pointerId: number; canvas: SVGSVGElement; points: Point[] } | null>(
    null,
  );
  function releaseStroke() {
    const active = activeStroke.current;
    activeStroke.current = null;
    if (active?.canvas.hasPointerCapture?.(active.pointerId))
      active.canvas.releasePointerCapture(active.pointerId);
  }
  useEffect(() => {
    const queue = {
      active: true,
      running: false,
      pending: null,
    } as typeof recognitionQueue.current;
    recognitionQueue.current = queue;
    setRecognizing(false);
    closingRef.current = false;
    setClosing(false);
    submittingRef.current = false;
    setSubmitting(false);
    return () => {
      queue.active = false;
      queue.pending = null;
      recognitionRevision.current++;
      submissionRevision.current++;
      submittingRef.current = false;
      releaseStroke();
    };
  }, [client]);
  async function closeHandwriting() {
    const queue = recognitionQueue.current;
    if (!queue.active || closingRef.current) return;
    if (activeStroke.current) {
      setNotice("请完成当前笔画后关闭手写识别板");
      return;
    }
    closingRef.current = true;
    setClosing(true);
    try {
      await client.close();
      if (queue.active && recognitionQueue.current === queue) {
        queue.active = false;
        queue.pending = null;
        recognitionRevision.current++;
        submissionRevision.current++;
        releaseStroke();
      }
    } catch {
      if (queue.active && recognitionQueue.current === queue) {
        setNotice("无法关闭手写识别板，请重试");
      }
    } finally {
      if (queue.active && recognitionQueue.current === queue) {
        closingRef.current = false;
        setClosing(false);
      }
    }
  }
  function recognize(nextStrokes: InkStroke[]) {
    if (!recognitionQueue.current.active || closingRef.current) return;
    const revision = ++recognitionRevision.current;
    setCandidates([]);
    setNotice("正在识别…");
    const recognizeInk = client.recognizeHandwriting;
    if (!recognizeInk) {
      setNotice("识别结果需由宿主提供");
      return;
    }
    const queue = recognitionQueue.current;
    queue.pending = { revision, strokes: nextStrokes };
    setRecognizing(true);
    if (queue.running) return;
    queue.running = true;
    void (async () => {
      try {
        while (queue.active && queue === recognitionQueue.current && queue.pending) {
          const request = queue.pending;
          queue.pending = null;
          if (request.revision !== recognitionRevision.current) continue;
          try {
            const result = await recognizeInk({ language: "zh-CN", strokes: request.strokes });
            if (request.revision !== recognitionRevision.current) continue;
            const nextCandidates = normalizeHandwritingCandidates(result.candidates);
            setCandidates(nextCandidates);
            setNotice(
              nextCandidates.length
                ? "选择候选可复制或输入"
                : platform === "windows"
                  ? "未识别到内容，请确认已安装中文手写包"
                  : "未识别到内容，请重写",
            );
          } catch {
            if (request.revision === recognitionRevision.current) {
              setCandidates([]);
              setNotice("手写识别失败，请确认手写模型或识别服务可用，然后重新识别");
            }
          }
        }
      } finally {
        queue.running = false;
        if (queue.active && queue === recognitionQueue.current) setRecognizing(false);
      }
    })();
  }
  function retryRecognition() {
    if (!recognitionQueue.current.active || closingRef.current) return;
    if (
      !strokes.length ||
      activeStroke.current ||
      recognitionQueue.current.running ||
      submittingRef.current
    )
      return;
    recognize(strokes);
  }
  function start(event: PointerEvent<SVGSVGElement>) {
    if (
      !recognitionQueue.current.active ||
      closingRef.current ||
      activeStroke.current ||
      event.isPrimary === false ||
      (event.button !== undefined && event.button !== 0)
    )
      return;
    if (strokes.length >= maxStrokes) {
      setNotice("笔画已达上限，请选择候选、撤销或重写");
      return;
    }
    event.currentTarget.focus({ preventScroll: true });
    recognitionRevision.current++;
    setCandidates([]);
    setNotice("书写中，松开后自动识别");
    const points = appendPointerSamples([], event);
    if (!points.length) return;
    activeStroke.current = { pointerId: event.pointerId, canvas: event.currentTarget, points };
    event.currentTarget.setPointerCapture?.(event.pointerId);
    setDrawing(points);
  }
  function move(event: PointerEvent<SVGSVGElement>) {
    const active = activeStroke.current;
    if (!active || active.pointerId !== event.pointerId) return;
    active.points = appendPointerSamples(active.points, event);
    setDrawing(active.points);
  }
  function recognizeRemaining(nextStrokes: InkStroke[]) {
    setCandidates([]);
    if (!nextStrokes.length) setNotice("请在左侧书写，松开鼠标后自动识别");
    else void recognize(nextStrokes);
  }
  function end(event: PointerEvent<SVGSVGElement>) {
    const active = activeStroke.current;
    if (!active || active.pointerId !== event.pointerId) return;
    active.points = appendPointerSamples(active.points, event, true);
    // Match the Windows handwriting panel: a click without movement is not an
    // ink stroke and must not trigger a recognition request.
    const nextStrokes =
      active.points.length >= 2 ? [...strokes, { points: active.points }] : strokes;
    releaseStroke();
    setStrokes(nextStrokes);
    if (active.points.length >= 2) setRedoStrokes([]);
    setDrawing([]);
    recognizeRemaining(nextStrokes);
  }
  function cancel(event: PointerEvent<SVGSVGElement>) {
    if (!activeStroke.current || activeStroke.current.pointerId !== event.pointerId) return;
    recognitionRevision.current++;
    releaseStroke();
    setDrawing([]);
    recognizeRemaining(strokes);
  }
  function undo() {
    if (!recognitionQueue.current.active || closingRef.current) return;
    const wasDrawing = activeStroke.current !== null;
    recognitionRevision.current++;
    recognitionQueue.current.pending = null;
    releaseStroke();
    setDrawing([]);
    if (wasDrawing) {
      // Cancel only the unfinished stroke; completed ink stays available.
      recognizeRemaining(strokes);
      return;
    }
    if (!strokes.length) return;
    setRedoStrokes((current) => [...current, strokes[strokes.length - 1]]);
    const nextStrokes = strokes.slice(0, -1);
    setStrokes(nextStrokes);
    recognizeRemaining(nextStrokes);
  }
  function redo() {
    if (!recognitionQueue.current.active || closingRef.current) return;
    if (activeStroke.current || !redoStrokes.length || strokes.length >= maxStrokes) return;
    recognitionRevision.current++;
    const nextStrokes = [...strokes, redoStrokes[redoStrokes.length - 1]];
    setRedoStrokes((current) => current.slice(0, -1));
    setStrokes(nextStrokes);
    recognizeRemaining(nextStrokes);
  }
  function clear() {
    if (!recognitionQueue.current.active || closingRef.current) return;
    recognitionRevision.current++;
    recognitionQueue.current.pending = null;
    releaseStroke();
    setStrokes([]);
    setRedoStrokes([]);
    setDrawing([]);
    setCandidates([]);
    setNotice("请在左侧书写，松开鼠标后自动识别");
  }
  function editInk(event: import("react").KeyboardEvent<HTMLElement>) {
    if (
      event.defaultPrevented ||
      event.nativeEvent.isComposing ||
      event.keyCode === 229 ||
      event.altKey
    )
      return;
    const target = event.target as HTMLElement;
    if (target.closest("input, textarea, select, [contenteditable=true]")) return;
    const key = event.key.toLowerCase();
    if ((event.ctrlKey || event.metaKey) && (key === "z" || key === "y")) {
      event.preventDefault();
      if (key === "y" || event.shiftKey) redo();
      else undo();
    } else if (event.key === "Escape" && activeStroke.current) {
      event.preventDefault();
      undo();
    }
  }
  function navigateCandidates(event: import("react").KeyboardEvent<HTMLDivElement>) {
    if (
      event.defaultPrevented ||
      event.nativeEvent.isComposing ||
      event.keyCode === 229 ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      event.shiftKey ||
      closingRef.current ||
      submittingRef.current
    )
      return;
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key))
      return;
    const buttons = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>(
        ".handwriting-candidate-submit:not(:disabled)",
      ),
    );
    const index = buttons.indexOf(event.target as HTMLButtonElement);
    if (index < 0) return;
    // The candidate grid has four columns; keep a partial final row reachable.
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? buttons.length - 1
          : index +
            (event.key === "ArrowLeft"
              ? -1
              : event.key === "ArrowRight"
                ? 1
                : event.key === "ArrowUp"
                  ? -4
                  : 4);
    event.preventDefault();
    const target = buttons[clamp(next, 0, buttons.length - 1)];
    target.focus({ preventScroll: true });
    target.scrollIntoView({ block: "nearest", inline: "nearest" });
  }
  async function chooseCandidate(candidate: string, copyOnly = false) {
    if (
      !recognitionQueue.current.active ||
      closingRef.current ||
      submittingRef.current ||
      !candidates.includes(candidate)
    )
      return;
    const action = copyOnly ? client.copyHandwritingCandidate : client.submitHandwritingCandidate;
    if (!action) {
      setNotice(`已选择：${candidate}（等待宿主提交能力）`);
      return;
    }
    const revision = ++submissionRevision.current;
    const inkRevision = recognitionRevision.current;
    submittingRef.current = true;
    setSubmitting(true);
    setNotice(copyOnly ? "正在复制候选…" : "正在提交候选…");
    try {
      await action(candidate);
      if (revision === submissionRevision.current && inkRevision === recognitionRevision.current) {
        setNotice(copyOnly ? `已复制：${candidate}` : `已提交：${candidate}`);
      }
    } catch {
      if (revision === submissionRevision.current && inkRevision === recognitionRevision.current) {
        setNotice(
          copyOnly
            ? "复制失败，候选和笔画已保留，请重试"
            : "提交失败，可复制候选后手动粘贴，或重试",
        );
      }
    } finally {
      if (revision === submissionRevision.current) {
        submittingRef.current = false;
        setSubmitting(false);
      }
    }
  }

  const renderStrokes = [...strokes, ...(drawing.length ? [{ points: drawing }] : [])];
  return (
    <main
      className={`native-panel ${surface.handwritingPanel}`}
      onKeyDown={editInk}
      data-panel-theme={theme}
      aria-label="手写识别板"
    >
      <header className={`native-panel-header ${surface.panelHeader}`} {...drag}>
        <span>水杉手写识别板</span>
        <button
          type="button"
          aria-label="关闭"
          disabled={closing}
          onClick={() => void closeHandwriting()}
        >
          ×
        </button>
      </header>
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
          <div className={surface.handwritingActions}>
            <button
              type="button"
              onClick={undo}
              disabled={closing || (!strokes.length && !drawing.length)}
              aria-keyshortcuts="Control+z Meta+z"
            >
              ↶ 撤销
            </button>
            <button
              type="button"
              onClick={redo}
              disabled={closing || !redoStrokes.length || drawing.length > 0}
              aria-keyshortcuts="Control+Shift+z Meta+Shift+z Control+y"
            >
              ↷ 重做
            </button>
            <button
              type="button"
              onClick={clear}
              disabled={closing || (!strokes.length && !drawing.length && !redoStrokes.length)}
            >
              × 重写
            </button>
            {client.recognizeHandwriting && (
              <button
                type="button"
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
            <button
              type="button"
              aria-pressed={effectiveMode === "copy"}
              disabled={closing || submitting || !client.copyHandwritingCandidate}
              onClick={() => selectActivation("copy")}
            >
              复制
            </button>
            <button
              type="button"
              aria-pressed={effectiveMode === "input"}
              disabled={closing || submitting || !client.submitHandwritingCandidate}
              onClick={() => selectActivation("input")}
            >
              输入
            </button>
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
          <p role="status">{notice}</p>
        </section>
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
  const [inputLevel, setInputLevel] = useState<number | undefined>();
  const [language, setLanguage] = useState("zh-CN");
  const [text, setText] = useState("");
  const exceedsSubmitLimit =
    client.maxSubmitBytes !== undefined && utf8ByteLength(text) > client.maxSubmitBytes;
  const textRevision = useRef(0);
  function updateText(value: string) {
    textRevision.current++;
    setText(value);
  }
  const [submitting, setSubmitting] = useState(false);
  const [copying, setCopying] = useState(false);
  const submittingRef = useRef(false);
  const submissionRevision = useRef(0);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [stopping, setStopping] = useState(false);
  const stoppingRef = useRef(false);
  const recognitionRevision = useRef(0);
  const [notice, setNotice] = useState("点击开始后由宿主录音并进行语音识别");
  const drag = usePanelDrag(client, () => setNotice("无法移动窗口，请重试。"));

  useEffect(() => {
    let active = true;
    if (!client.loadVoiceLanguage) return;
    void client
      .loadVoiceLanguage()
      .then((next) => {
        if (active && validVoiceLanguage(next)) setLanguage(next);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    let active = true;
    if (!client.rememberInputTarget) return;
    void client.rememberInputTarget().catch(() => {
      if (active) setNotice("未能记录前台输入窗口");
    });
    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    submittingRef.current = false;
    setSubmitting(false);
    setCopying(false);
    setBusy(false);
    stoppingRef.current = false;
    setStopping(false);
    return () => {
      submissionRevision.current++;
      submittingRef.current = false;
      recognitionRevision.current++;
      const wasBusy = busyRef.current;
      busyRef.current = false;
      if (wasBusy && client.cancelVoice) void client.cancelVoice().catch(() => undefined);
    };
  }, [client]);

  useEffect(() => {
    if (!client.onVoiceUpdate) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void client
      .onVoiceUpdate((update) => {
        if (!active || !busyRef.current) return;
        if (update.level !== undefined) {
          if (
            !stoppingRef.current &&
            Number.isFinite(update.level) &&
            update.level >= 0 &&
            update.level <= 1
          ) {
            setInputLevel(update.level);
          }
          return;
        }
        if (update.phase) {
          const labels = {
            recording: "正在录音…",
            recognizing: "正在识别…",
            polishing: "正在润色…",
          };
          setNotice(labels[update.phase]);
          if (update.phase !== "recording") {
            stoppingRef.current = true;
            setStopping(true);
          }
          return;
        }
        updateText(update.text);
        setNotice(
          update.final
            ? update.text
              ? "识别完成，点击提交即可输入"
              : "没有识别到内容"
            : stoppingRef.current
              ? "正在识别…"
              : "正在录音并识别…",
        );
      })
      .then((stop) => {
        if (active) unlisten = stop;
        else stop();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      unlisten?.();
    };
  }, [client]);

  async function recognize() {
    if (busyRef.current || submittingRef.current) return;
    if (!client.recognizeVoice) {
      setNotice("当前宿主未提供语音识别能力");
      return;
    }
    if (!validVoiceLanguage(language)) {
      setNotice("请输入有效的识别语言码");
      return;
    }
    const revision = ++recognitionRevision.current;
    busyRef.current = true;
    setBusy(true);
    setInputLevel(undefined);
    updateText("");
    setNotice("正在录音并识别…");
    try {
      const result = await client.recognizeVoice(language);
      if (revision !== recognitionRevision.current) return;
      updateText(result.text);
      setNotice(result.text ? "识别完成，点击提交即可输入" : "没有识别到内容");
    } catch {
      if (revision === recognitionRevision.current) setNotice("语音识别失败，请确认录音服务已启动");
    } finally {
      if (revision === recognitionRevision.current) {
        busyRef.current = false;
        setBusy(false);
        stoppingRef.current = false;
        setStopping(false);
      }
    }
  }

  async function submit(copyOnly = false) {
    const send = copyOnly ? client.copyText : (client.sendVoiceText ?? client.sendText);
    if (!text || !send || busyRef.current || submittingRef.current) return;
    if (!copyOnly && exceedsSubmitLimit) {
      setNotice("内容超过单次提交长度，请精简或复制结果后手动粘贴");
      return;
    }
    const revision = ++submissionRevision.current;
    const submittedTextRevision = textRevision.current;
    submittingRef.current = true;
    setSubmitting(true);
    setCopying(copyOnly);
    setNotice(copyOnly ? "正在复制…" : "正在提交…");
    try {
      await send(text);
      if (revision !== submissionRevision.current) return;
      if (submittedTextRevision !== textRevision.current) {
        setNotice(
          copyOnly ? "上一版内容已复制，当前内容已保留" : "上一版内容已提交，当前内容已保留",
        );
        return;
      }
      if (copyOnly) {
        setNotice("识别结果已复制，文本已保留");
      } else {
        setNotice(client.submitNotice ?? `已提交：${text}`);
        updateText("");
      }
    } catch {
      if (revision === submissionRevision.current)
        setNotice(copyOnly ? "复制失败，识别结果已保留" : "提交失败，前台输入窗口可能已关闭");
    } finally {
      if (revision === submissionRevision.current) {
        submittingRef.current = false;
        setSubmitting(false);
        setCopying(false);
      }
    }
  }

  async function stop() {
    if (!client.stopVoice) {
      await cancel();
      return;
    }
    if (!busyRef.current || stoppingRef.current) return;
    const revision = recognitionRevision.current;
    stoppingRef.current = true;
    setStopping(true);
    setNotice("正在完成识别…");
    try {
      await client.stopVoice();
    } catch {
      if (revision === recognitionRevision.current && busyRef.current) {
        stoppingRef.current = false;
        setStopping(false);
        setNotice("停止录音失败，请重试或取消录音");
      }
    }
  }

  async function cancel() {
    recognitionRevision.current++;
    busyRef.current = false;
    // Cancellation is immediate even if the provider acknowledgement is delayed.
    setBusy(false);
    stoppingRef.current = false;
    setStopping(false);
    setNotice("录音已停止");
    try {
      await client.cancelVoice?.();
    } catch {
      /* provider may already have stopped */
    }
  }

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.key !== "Escape" ||
        event.isComposing ||
        event.ctrlKey ||
        event.altKey ||
        event.metaKey ||
        event.shiftKey ||
        !busyRef.current ||
        !client.cancelVoice
      )
        return;
      event.preventDefault();
      event.stopPropagation();
      void cancel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [client]);

  async function close() {
    submissionRevision.current++;
    recognitionRevision.current++;
    const wasBusy = busyRef.current;
    busyRef.current = false;
    setBusy(false);
    stoppingRef.current = false;
    setStopping(false);
    if (wasBusy && client.cancelVoice) {
      try {
        await client.cancelVoice();
      } catch {
        /* close even if provider is gone */
      }
    }
    await client.close();
  }

  return (
    <main
      className={`native-panel ${surface.voicePanel}`}
      data-panel-theme={theme}
      aria-label="语音输入"
    >
      <header className={`native-panel-header ${surface.panelHeader}`} {...drag}>
        <span>水杉语音输入</span>
        <button type="button" aria-label="关闭" onClick={() => void close()}>
          ×
        </button>
      </header>
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
            <option value="zh-cn">中文（普通话）</option>
            <option value="en">English</option>
            <option value="ja">日本語</option>
            <option value="auto">自动识别</option>
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
        <button
          type="button"
          onClick={() => {
            updateText("");
            setNotice("识别结果已清空");
          }}
          disabled={!text || busy}
        >
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
  const [inputAvailable, setInputAvailable] = useState(
    Boolean(client.sendText && !client.canSendText),
  );
  const [confirmDisable, setConfirmDisable] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [search, setSearch] = useState("");
  const [items, setItems] = useState<CloudClipboardItem[]>([]);
  const [draft, setDraft] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [busy, setBusy] = useState(false);
  const refreshRevision = useRef(0);
  const busyRef = useRef(false);
  const draftRevision = useRef(0);
  const searchRef = useRef("");
  const [notice, setNotice] = useState("只上传你明确选择的内容");

  async function run(action: (revision: number) => Promise<void>, failure: string) {
    if (busyRef.current) return;
    const revision = ++refreshRevision.current;
    busyRef.current = true;
    setBusy(true);
    try {
      await action(revision);
    } catch {
      if (revision === refreshRevision.current) setNotice(failure);
    } finally {
      if (revision === refreshRevision.current) {
        busyRef.current = false;
        setBusy(false);
      }
    }
  }

  async function load(revision: number, nextSearch: string) {
    let result;
    try {
      result = await client.request({ operation: "list", search: nextSearch });
    } catch (error) {
      if (revision === refreshRevision.current) {
        setItems([]);
        setLoaded(false);
        setConfirmDisable(false);
      }
      throw error;
    }
    if (revision !== refreshRevision.current) return;
    setItems(cloudClipboardItems(result));
    setLoaded(true);
    if (typeof result.enabled === "boolean") setEnabled(result.enabled);
    setNotice("云剪贴板已刷新");
  }

  function refresh(nextSearch = searchRef.current) {
    return run((revision) => load(revision, nextSearch), "无法访问云剪贴板服务");
  }

  useEffect(() => {
    let active = true;
    busyRef.current = false;
    setItems([]);
    setLoaded(false);
    setInputAvailable(Boolean(client.sendText && !client.canSendText));
    if (client.canSendText)
      void client
        .canSendText()
        .then((available) => {
          if (active) setInputAvailable(available && Boolean(client.sendText));
        })
        .catch(() => {
          if (active) setInputAvailable(false);
        });
    if (client.rememberInputTarget)
      void client.rememberInputTarget().catch(() => {
        if (active) setNotice("未能记录前台输入窗口");
      });
    void refresh(searchRef.current);
    return () => {
      active = false;
      refreshRevision.current++;
      busyRef.current = false;
    };
  }, [client]);

  function add() {
    if (!enabled || draft.trim().length === 0 || draft.length > 4000) {
      if (draft.trim().length === 0) setNotice("请输入要上传的文本");
      else if (draft.length > 4000) setNotice("上传内容不能超过 4000 个 UTF-16 单元");
      return;
    }
    const uploadedRevision = draftRevision.current;
    return run(async (revision) => {
      await client.request({ operation: "add", text: draft });
      if (revision !== refreshRevision.current) return;
      if (uploadedRevision === draftRevision.current) setDraft("");
      try {
        await load(revision, searchRef.current);
      } catch {
        if (revision === refreshRevision.current) setNotice("已上传，但历史刷新失败，请点击刷新");
      }
    }, "上传失败，请确认账户 provider 已连接");
  }

  function remove(id: string) {
    return run(async (revision) => {
      await client.request({ operation: "delete", id });
      if (revision !== refreshRevision.current) return;
      try {
        await load(revision, searchRef.current);
      } catch {
        if (revision === refreshRevision.current) setNotice("已删除，但历史刷新失败，请点击刷新");
      }
    }, "删除失败");
  }

  function toggle(confirmed = false) {
    if (enabled && !confirmed) {
      setConfirmDisable(true);
      return;
    }
    setConfirmDisable(false);
    const next = !enabled;
    return run(async (revision) => {
      const result = await client.request({ operation: "set_enabled", enabled: next });
      if (revision !== refreshRevision.current) return;
      const effective = typeof result.enabled === "boolean" ? result.enabled : next;
      setEnabled(effective);
      if (!effective) setItems([]);
      setNotice(effective ? "云剪贴板已开启" : "云剪贴板已关闭");
    }, "更新云剪贴板设置失败");
  }

  function choose(item: CloudClipboardItem, copyOnly = false) {
    copyOnly = copyOnly || !inputAvailable;
    const send = copyOnly ? client.copyText : client.sendText;
    if (!send) {
      setNotice("当前宿主未提供此操作");
      return;
    }
    return run(
      async (revision) => {
        await send(item.text);
        if (revision === refreshRevision.current)
          setNotice(copyOnly ? "已复制到本机剪贴板" : `已输入：${item.text}`);
      },
      copyOnly ? "复制失败，请重试" : "提交失败，可复制后手动粘贴",
    );
  }

  return (
    <main className={`native-panel ${cloud.clipboardPanel}`} aria-label="云剪贴板">
      <CloudPanelHeader title="水杉云剪贴板" onClose={() => void client.close()} />
      <div className={cloud.clipboardBody}>
        <p className={cloud.clipboardNote}>只上传你明确选择的文本，不自动读取本地剪贴板。</p>
        <label className={cloud.clipboardToggle}>
          <span>启用云剪贴板</span>
          <input
            type="checkbox"
            checked={enabled}
            onChange={() => void toggle()}
            disabled={busy || !loaded}
          />
        </label>
        {confirmDisable && (
          <div
            className={cloud.clipboardConfirm}
            role="alertdialog"
            aria-label="关闭云剪贴板"
            aria-modal="true"
          >
            <p className="m-0">关闭云剪贴板会删除云端历史，是否继续？</p>
            <button
              type="button"
              className={cloud.clipboardDelete}
              disabled={busy}
              onClick={() => void toggle(true)}
            >
              确认关闭并删除历史
            </button>
            <button
              type="button"
              className={cloud.clipboardButton}
              onClick={() => setConfirmDisable(false)}
            >
              取消
            </button>
          </div>
        )}
        <div className={cloud.clipboardSearchRow}>
          <input
            className={cloud.clipboardInput}
            aria-label="搜索云端历史"
            value={search}
            onChange={(event) => {
              searchRef.current = event.target.value;
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
              draftRevision.current++;
              setDraft(event.target.value);
            }}
            placeholder="输入要上传的文本"
            rows={3}
          />
          <small className={cloud.clipboardCount(draft.length > 4000)}>{draft.length} / 4000</small>
          <button
            type="button"
            className={cloud.clipboardSubmit}
            onClick={() => void add()}
            disabled={!enabled || draft.trim().length === 0 || draft.length > 4000 || busy}
          >
            上传明确选择的文本
          </button>
        </div>
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
        <p className={cloud.clipboardNote} role="status">
          {notice}
        </p>
      </div>
    </main>
  );
}

const cloudDictionaryKinds: [CloudDictionaryKind, string][] = [
  ["pinyin", "拼音"],
  ["wubi", "五笔"],
  ["quick", "快捷短语"],
  ["english", "英文"],
];

export function CloudDictionaryPanel({ client }: { client: CloudDictionaryPanelClient }) {
  const { confirm, confirmation } = useConfirm();
  const [kind, setKind] = useState<CloudDictionaryKind>("pinyin");
  const [search, setSearch] = useState("");
  const [offset, setOffset] = useState(0);
  const [entries, setEntries] = useState<CloudDictionaryEntry[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [form, setForm] = useState<{
    entry: CloudDictionaryEntry | null;
    code: string;
    word: string;
    weight: number;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("管理当前账号的云端词条");
  const refreshRevision = useRef(0);
  const busyRef = useRef(false);
  const searchRef = useRef("");
  const mounted = useRef(true);

  useEffect(
    () => () => {
      mounted.current = false;
    },
    [],
  );

  async function run(action: (revision: number) => Promise<void>, failure: string) {
    if (!mounted.current || busyRef.current) return;
    const revision = ++refreshRevision.current;
    busyRef.current = true;
    setBusy(true);
    try {
      await action(revision);
    } catch {
      if (mounted.current && revision === refreshRevision.current) setNotice(failure);
    } finally {
      if (mounted.current && revision === refreshRevision.current) {
        busyRef.current = false;
        setBusy(false);
      }
    }
  }

  async function load(
    revision: number,
    nextOffset: number,
    nextSearch: string,
    nextKind: CloudDictionaryKind,
  ) {
    let result;
    try {
      result = await client.request({
        operation: "list",
        kind: nextKind,
        offset: nextOffset,
        search: nextSearch,
      });
    } catch (error) {
      if (revision === refreshRevision.current) {
        setEntries([]);
        setHasMore(false);
        setForm(null);
      }
      throw error;
    }
    if (revision !== refreshRevision.current) return;
    setEntries(cloudDictionaryEntries<CloudDictionaryEntry>(result));
    setOffset(typeof result.offset === "number" ? result.offset : nextOffset);
    setHasMore(result.has_more === true);
  }

  function refresh(nextOffset = 0, nextSearch = searchRef.current, nextKind = kind) {
    return run(async (revision) => {
      await load(revision, nextOffset, nextSearch, nextKind);
      if (revision === refreshRevision.current) setNotice("云词典已刷新");
    }, "无法访问云词典服务，请确认 provider 已连接");
  }

  useEffect(() => {
    busyRef.current = false;
    setEntries([]);
    setOffset(0);
    setHasMore(false);
    setForm(null);
    void refresh(0);
    return () => {
      refreshRevision.current++;
    };
  }, [client]);

  function beginAdd() {
    if (!busyRef.current) setForm({ entry: null, code: "", word: "", weight: 100000 });
  }
  function beginEdit(entry: CloudDictionaryEntry) {
    if (!busyRef.current)
      setForm({ entry, code: entry.code, word: entry.word, weight: entry.weight });
  }

  async function reloadAfterMutation(revision: number, success: string) {
    if (revision !== refreshRevision.current) return;
    try {
      await load(revision, 0, searchRef.current, kind);
      if (revision === refreshRevision.current) setNotice(success);
    } catch {
      if (revision === refreshRevision.current) {
        setEntries([]);
        setOffset(0);
        setHasMore(false);
        setNotice(`${success}，但列表刷新失败，请重新查询`);
      }
    }
  }

  function save() {
    if (busyRef.current) return;
    if (!form || !form.code.trim() || !form.word.trim()) {
      setNotice("编码和词条不能为空");
      return;
    }
    return run(async (revision) => {
      const code = form.code.trim();
      const word = form.word;
      if (form.entry) {
        await client.request({
          operation: "update",
          kind,
          id: form.entry.id,
          code,
          word,
          weight: form.weight,
          revision: form.entry.revision,
        });
      } else {
        await client.request({ operation: "add", kind, code, word, weight: form.weight });
      }
      if (revision !== refreshRevision.current) return;
      setForm(null);
      await reloadAfterMutation(revision, "云词条已保存");
    }, "云词条保存失败，请刷新后重试");
  }

  function remove(entry: CloudDictionaryEntry) {
    return run(async (revision) => {
      await client.request({ operation: "delete", kind, id: entry.id, revision: entry.revision });
      if (revision !== refreshRevision.current) return;
      setForm((current) => (current?.entry?.id === entry.id ? null : current));
      await reloadAfterMutation(revision, "云词条已删除");
    }, "云词条删除失败，请刷新后重试");
  }

  async function confirmRemove(entry: CloudDictionaryEntry) {
    if (!mounted.current || busyRef.current) return;
    const confirmed = await confirm({
      title: "删除云词条",
      message: `“${entry.word}”只会从云端删除，本机词库不会改变。`,
      confirmLabel: "删除",
      danger: true,
    });
    // Re-checked after the answer: the dialog is not instant, and another action may have started
    // while it was open.
    if (!confirmed || !mounted.current || busyRef.current) return;
    void remove(entry);
  }

  async function downloadToLocal(entry: CloudDictionaryEntry) {
    if (!mounted.current || busyRef.current || !client.downloadToLocal) return;
    const confirmed = await confirm({
      title: "加入本机词典",
      message: `云词条“${entry.word}”会被加入本机个人词典。`,
      confirmLabel: "加入",
    });
    if (!confirmed || !mounted.current || busyRef.current || !client.downloadToLocal) return;
    const download = client.downloadToLocal;
    return run(async (revision) => {
      await download(entry);
      if (revision === refreshRevision.current) setNotice("云词条已加入本机词典队列");
    }, "加入本机词典失败，请重试");
  }

  function changeKind(next: CloudDictionaryKind) {
    if (busyRef.current || next === kind) return;
    setKind(next);
    setForm(null);
    setOffset(0);
    setEntries([]);
    setHasMore(false);
    void refresh(0, searchRef.current, next);
  }
  return (
    <main className={`native-panel ${cloud.dictionaryPanel}`} aria-label="云词典">
      {confirmation}
      <CloudPanelHeader title="水杉云词典" onClose={() => void client.close()} />
      <div className={cloud.dictionaryBody}>
        <p className={cloud.dictionaryNote}>
          管理当前账号的云端词条。修改需要 provider 提供登录态和同步服务。
        </p>
        <div className={cloud.dictionaryKindTabs} role="tablist" aria-label="云词库类型">
          {cloudDictionaryKinds.map(([value, label]) => (
            <button
              key={value}
              type="button"
              role="tab"
              aria-selected={kind === value}
              className={cloud.dictionaryKindTab(kind === value)}
              onClick={() => changeKind(value)}
              disabled={busy}
            >
              {label}
            </button>
          ))}
        </div>
        <div className={cloud.dictionaryToolbar}>
          <label className={`${cloud.dictionaryField} ${cloud.dictionaryDesktopOnlyField}`}>
            词库
            <select
              className={cloud.dictionaryInput}
              aria-label="词库类型"
              value={kind}
              onChange={(event) => changeKind(event.target.value as CloudDictionaryKind)}
              disabled={busy}
            >
              {cloudDictionaryKinds.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label className={cloud.dictionarySearch}>
            搜索
            <input
              className={cloud.dictionaryInput}
              aria-label="搜索云词条"
              value={search}
              onChange={(event) => {
                searchRef.current = event.target.value;
                setSearch(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") void refresh(0);
              }}
              placeholder="词条或编码"
            />
          </label>
          <button type="button" onClick={() => void refresh(0)} disabled={busy}>
            查询
          </button>
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
        </div>
        {form && (
          <CloudDictionaryEntryForm
            value={form}
            busy={busy}
            onChange={(patch) => setForm((current) => (current ? { ...current, ...patch } : null))}
            onSave={() => void save()}
            onCancel={() => setForm(null)}
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
              <article className={cloud.dictionaryItem} key={entry.id}>
                <button
                  type="button"
                  className={cloud.dictionaryItemMain}
                  aria-label={`编辑云词条 ${entry.word}`}
                  onClick={() => beginEdit(entry)}
                  disabled={busy}
                >
                  <strong>{entry.word}</strong>
                  <small>
                    {entry.code} · 权重 {entry.weight}
                  </small>
                </button>
                <div className={cloud.dictionaryItemActions}>
                  {client.downloadToLocal && (
                    <button
                      type="button"
                      className="secondary"
                      aria-label={`下载到本机 ${entry.word}`}
                      onClick={() => void downloadToLocal(entry)}
                      disabled={busy}
                    >
                      下载到本机
                    </button>
                  )}
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => beginEdit(entry)}
                    disabled={busy}
                  >
                    编辑
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => confirmRemove(entry)}
                    disabled={busy}
                  >
                    删除
                  </button>
                </div>
              </article>
            ))
          ) : (
            <p className={cloud.dictionaryEmpty}>暂无词条</p>
          )}
        </div>
        <div className={cloud.dictionaryPagination}>
          <button
            className={cloud.dictionaryButton}
            type="button"
            onClick={() => void refresh(Math.max(0, offset - 100))}
            disabled={busy || offset === 0}
          >
            上一页
          </button>
          <span>第 {Math.floor(offset / 100) + 1} 页</span>
          <button
            className={cloud.dictionaryButton}
            type="button"
            onClick={() => void refresh(offset + 100)}
            disabled={busy || !hasMore}
          >
            下一页
          </button>
        </div>
        <p className={cloud.dictionaryNote} role="status">
          {notice}
        </p>
      </div>
    </main>
  );
}

/** File transfer lives on its own page so the dictionary list stays focused on entries. */
export function CloudDictionaryFilesPanel({ client }: { client: CloudDictionaryPanelClient }) {
  const { confirm, confirmation } = useConfirm();
  const [kind, setKind] = useState<CloudDictionaryKind>("pinyin");
  const [format, setFormat] = useState<CloudDictionaryFileFormat>("standard");
  const [file, setFile] = useState<{ name: string; text: string; bytes: number } | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("选择 TSV 文件后确认上传；导出不会改变云端内容");
  const [snapshotBusy, setSnapshotBusy] = useState(false);
  const [restorePreview, setRestorePreview] = useState<{
    text: string;
    snapshot: CloudDictionarySnapshotMetadata;
    expectedRevision: number;
  } | null>(null);
  const requestRevision = useRef(0);
  const busyRef = useRef(false);
  const mounted = useRef(true);
  const lifecycleRevision = useRef(0);

  useEffect(() => {
    const current = ++lifecycleRevision.current;
    requestRevision.current++;
    busyRef.current = false;
    setBusy(false);
    setSnapshotBusy(false);
    setRestorePreview(null);
    return () => {
      if (current === lifecycleRevision.current) lifecycleRevision.current++;
      requestRevision.current++;
      busyRef.current = false;
      if (client.snapshotNative) void client.request({ operation: "snapshot_restore_cancel" });
    };
  }, [client]);

  useEffect(() => {
    return () => {
      mounted.current = false;
    };
  }, []);

  async function run(action: (revision: number) => Promise<void>, failure: string) {
    if (busyRef.current) return;
    const revision = ++requestRevision.current;
    busyRef.current = true;
    setBusy(true);
    try {
      await action(revision);
    } catch {
      if (revision === requestRevision.current) setNotice(failure);
    } finally {
      if (revision === requestRevision.current) {
        busyRef.current = false;
        setBusy(false);
      }
    }
  }

  function changeKind(next: CloudDictionaryKind) {
    if (busyRef.current || next === kind) return;
    setKind(next);
    setFile(null);
    if (next !== "pinyin" && format === "hans") setFormat("standard");
  }

  function chooseFile(selected: File) {
    if (busyRef.current) return;
    if (selected.size === 0 || selected.size > 65536) {
      setFile(null);
      setNotice("导入文件必须大于 0 且不超过 64 KiB");
      return;
    }
    void run(async (revision) => {
      let text: string;
      try {
        // The same reader the local dictionary import uses. `File.text()` decodes as UTF-8 and
        // nothing else, which is wrong twice over for dictionary files: a UTF-16 one came out as
        // NULs and was refused, and a GB18030 one - which Windows tools still write - decoded to
        // replacement characters with no NUL anywhere, so it passed the check below and uploaded
        // a cloud dictionary of `\ufffd`. Reading it here the way the other panel does means one
        // file behaves the same in both.
        text = await readDictionaryFile(selected, 65536);
      } catch {
        throw new Error("invalid file");
      }
      if (revision !== requestRevision.current) return;
      if (!text || text.includes("\u0000")) throw new Error("invalid file");
      setFile({ name: selected.name || "dictionary.tsv", text, bytes: selected.size });
      setNotice("文件已读取；确认后才会写入云端");
    }, "无法读取文件，请确认是大小不超过 64 KiB 的文本词库");
  }

  async function importFile() {
    const prepared = file;
    if (!prepared || busyRef.current) return;
    const formatName =
      format === "hans" ? "汉字自动注音" : format === "windows" ? "Windows TSV" : "标准 TSV";
    const kindName =
      kind === "pinyin"
        ? "拼音"
        : kind === "wubi"
          ? "五笔"
          : kind === "quick"
            ? "快捷短语"
            : "英文";
    const confirmed = await confirm({
      title: "导入词库",
      message: `按“${formatName}”导入${kindName}词库。`,
      confirmLabel: "导入",
    });
    if (!confirmed || busyRef.current) return;
    void run(async (revision) => {
      await client.request({ operation: "import", kind, format, text: prepared.text });
      if (revision !== requestRevision.current) return;
      setFile(null);
      setNotice("云词库已导入；如需影响本机输入，请另行应用到本机");
    }, "导入失败，请检查 TSV 文件格式");
  }

  function exportDictionary() {
    if (format === "hans") return;
    void run(async (revision) => {
      const result = await client.request({ operation: "export", kind, format });
      if (revision !== requestRevision.current) return;
      if (client.exportNative) {
        setNotice(result.saved === true ? "云词库已导出" : "已取消导出");
        return;
      }
      const text = cloudResponseText(result);
      if (typeof text !== "string") throw new Error("provider returned no file");
      const anchor = document.createElement("a");
      const url = URL.createObjectURL(new Blob([text], { type: "text/plain;charset=utf-8" }));
      anchor.href = url;
      anchor.download = result.filename || `msime-${kind}-dictionary.tsv`;
      document.body.appendChild(anchor);
      try {
        anchor.click();
        setNotice("云词库已导出");
      } finally {
        anchor.remove();
        window.setTimeout(() => URL.revokeObjectURL(url), 1000);
      }
    }, "导出失败，请确认 provider 已连接");
  }

  async function exportSnapshot() {
    if (!mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    setSnapshotBusy(true);
    try {
      const result = await client.request({ operation: "snapshot_export" });
      if (!mounted.current || lifecycle !== lifecycleRevision.current) return;
      if (client.snapshotNative && typeof result.saved === "boolean") {
        setNotice(result.saved ? "完整云词库快照已导出" : "已取消导出");
        return;
      }
      const text = cloudResponseText(result);
      if (typeof text !== "string" || !text) throw new Error("provider returned no snapshot");
      const anchor = document.createElement("a");
      const url = URL.createObjectURL(
        new Blob([text], { type: "application/x-ndjson;charset=utf-8" }),
      );
      anchor.href = url;
      anchor.download = result.filename || "msime-dictionary-snapshot.ndjson";
      document.body.appendChild(anchor);
      try {
        anchor.click();
        setNotice("完整云词库快照已导出");
      } finally {
        anchor.remove();
        window.setTimeout(() => URL.revokeObjectURL(url), 1000);
      }
    } catch {
      if (mounted.current && lifecycle === lifecycleRevision.current)
        setNotice("导出完整云词库快照失败");
    } finally {
      if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
    }
  }

  async function chooseRestoreSnapshot(selected: File) {
    if (!mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    if (selected.size === 0 || selected.size > 512 * 1024 * 1024) {
      setNotice("快照文件必须大于 0 且不超过 512 MiB");
      return;
    }
    setSnapshotBusy(true);
    try {
      const text = client.snapshotNative ? "" : await selected.text();
      const result = await client.request({ operation: "snapshot_restore_preview", text });
      if (!mounted.current || lifecycle !== lifecycleRevision.current) return;
      if (!result.snapshot || typeof result.expectedRevision !== "number")
        throw new Error("invalid snapshot preview");
      if (client.snapshotNative && typeof result.previewToken !== "string")
        throw new Error("invalid native snapshot preview");
      const preparedText = client.snapshotNative ? result.previewToken! : text;
      setRestorePreview({
        text: preparedText,
        snapshot: result.snapshot,
        expectedRevision: result.expectedRevision,
      });
      setNotice("快照已校验，请确认后替换云端词库");
    } catch {
      if (mounted.current && lifecycle === lifecycleRevision.current)
        setNotice("无法校验快照，云端词库未改变");
    } finally {
      if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
    }
  }

  async function chooseNativeRestoreSnapshot() {
    if (!client.chooseSnapshotRestore || !mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    setRestorePreview(null);
    setSnapshotBusy(true);
    try {
      const result = await client.chooseSnapshotRestore();
      if (!mounted.current || lifecycle !== lifecycleRevision.current) return;
      if (result.saved === false) {
        setNotice("已取消选择快照");
        return;
      }
      if (
        !result.snapshot ||
        typeof result.expectedRevision !== "number" ||
        typeof result.previewToken !== "string"
      )
        throw new Error("invalid native snapshot preview");
      setRestorePreview({
        text: result.previewToken,
        snapshot: result.snapshot,
        expectedRevision: result.expectedRevision,
      });
      setNotice("快照已校验，请确认后替换云端词库");
    } catch {
      if (mounted.current && lifecycle === lifecycleRevision.current)
        setNotice("无法校验快照，云端词库未改变");
    } finally {
      if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
    }
  }

  async function abandonRestoreSnapshot() {
    if (!mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    setRestorePreview(null);
    if (client.snapshotNative) {
      setSnapshotBusy(true);
      try {
        await client.request({ operation: "snapshot_restore_cancel" });
      } finally {
        if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
      }
    }
  }

  async function restoreSnapshot() {
    if (!mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    const prepared = restorePreview;
    if (!prepared) return;
    const confirmed = await confirm({
      title: "替换云端词库",
      message: "此快照会替换全部云端词库和排序记录。",
      confirmLabel: "替换",
      danger: true,
    });
    if (!confirmed || !mounted.current || lifecycle !== lifecycleRevision.current) return;
    setSnapshotBusy(true);
    try {
      const result = await client.request(
        client.snapshotNative
          ? { operation: "snapshot_restore_native", token: prepared.text }
          : {
              operation: "snapshot_restore",
              text: prepared.text,
              expected_sha256: prepared.snapshot.sha256,
              revision: prepared.expectedRevision,
            },
      );
      if (!mounted.current || lifecycle !== lifecycleRevision.current) return;
      setRestorePreview(null);
      setNotice(`云端词库已恢复到新版本 ${result.revision ?? ""}`.trim());
    } catch {
      if (mounted.current && lifecycle === lifecycleRevision.current)
        setNotice("恢复失败，可能是云端版本已变化；云端词库未改变");
    } finally {
      if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
    }
  }

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
        <div className={cloud.dictionaryKindTabs} role="tablist" aria-label="云词库类型">
          {cloudDictionaryKinds.map(([value, label]) => (
            <button
              key={value}
              type="button"
              role="tab"
              aria-selected={kind === value}
              className={cloud.dictionaryKindTab(kind === value)}
              onClick={() => changeKind(value)}
              disabled={busy}
            >
              {label}
            </button>
          ))}
        </div>
        <div className={cloud.dictionaryToolbar}>
          <label className={`${cloud.dictionaryField} ${cloud.dictionaryDesktopOnlyField}`}>
            词库
            <select
              className={cloud.dictionaryInput}
              aria-label="词库类型"
              value={kind}
              onChange={(event) => changeKind(event.target.value as CloudDictionaryKind)}
              disabled={busy}
            >
              {cloudDictionaryKinds.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label className={cloud.dictionaryField}>
            文件格式
            <select
              className={cloud.dictionaryInput}
              aria-label="文件格式"
              value={format}
              onChange={(event) => setFormat(event.target.value as CloudDictionaryFileFormat)}
              disabled={busy}
            >
              <option value="standard">词在前（标准 TSV）</option>
              <option value="windows">编码在前（Windows TSV）</option>
              {kind === "pinyin" && <option value="hans">汉字自动注音（仅导入）</option>}
            </select>
          </label>
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

/** Apply the complete cloud snapshot through the platform's idle-boundary queue. */
export function CloudDictionaryApplyPanel({ client }: { client: CloudDictionaryPanelClient }) {
  const { confirm, confirmation } = useConfirm();
  const [localVersion, setLocalVersion] = useState<string | null>(null);
  const [request, setRequest] = useState<CloudDictionarySnapshotRequest | null>(null);
  const [preview, setPreview] = useState<CloudDictionarySnapshotMetadata | null>(null);
  const [previewToken, setPreviewToken] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("先获取本机词库版本，再下载并预览云端快照");
  const busyRef = useRef(false);
  const statusRevision = useRef(0);
  const lifecycleRevision = useRef(0);

  async function refreshStatus() {
    if (busyRef.current) return;
    const lifecycle = lifecycleRevision.current;
    const revision = ++statusRevision.current;
    try {
      const result = await client.request({ operation: "snapshot_status" });
      if (lifecycle !== lifecycleRevision.current || revision !== statusRevision.current) return;
      setLocalVersion(typeof result.localVersion === "string" ? result.localVersion : null);
      setRequest(cloudResponseRequest<CloudDictionarySnapshotRequest>(result));
    } catch {
      if (lifecycle === lifecycleRevision.current && revision === statusRevision.current)
        setNotice("无法读取本机词库状态，请确认键盘已启用");
    }
  }

  async function run(
    action: (revision: number, lifecycle: number) => Promise<void>,
    failure: string,
  ) {
    if (busyRef.current) return;
    const lifecycle = lifecycleRevision.current;
    const revision = ++statusRevision.current;
    busyRef.current = true;
    setBusy(true);
    try {
      await action(revision, lifecycle);
    } catch {
      if (lifecycle === lifecycleRevision.current && revision === statusRevision.current)
        setNotice(failure);
    } finally {
      if (lifecycle === lifecycleRevision.current && revision === statusRevision.current) {
        busyRef.current = false;
        setBusy(false);
      }
    }
  }

  function download() {
    if (!localVersion) {
      setNotice("尚未获取本机词库版本，请先打开水杉键盘");
      return;
    }
    void run(async (revision, lifecycle) => {
      const result = await client.request({ operation: "snapshot_preview" });
      if (lifecycle !== lifecycleRevision.current || revision !== statusRevision.current) return;
      if (!result.snapshot || typeof result.previewToken !== "string")
        throw new Error("invalid preview");
      setPreview(result.snapshot);
      setPreviewToken(result.previewToken);
      setNotice("云端快照已校验，请确认后替换本机词库");
    }, "下载云词库快照失败");
  }

  async function enqueue() {
    const token = previewToken;
    if (!token || !preview || busyRef.current) return;
    const lifecycle = lifecycleRevision.current;
    const confirmed = await confirm({
      title: "替换本机词库",
      message: "这份云端快照会替换本机个人词库和学习记录，输入法将在下一次空闲边界应用。",
      confirmLabel: "替换",
      danger: true,
    });
    if (!confirmed || busyRef.current || lifecycle !== lifecycleRevision.current) return;
    void run(async (revision, currentLifecycle) => {
      const result = await client.request({ operation: "snapshot_enqueue", token });
      if (currentLifecycle !== lifecycleRevision.current || revision !== statusRevision.current)
        return;
      setPreview(null);
      setPreviewToken(null);
      setRequest(cloudResponseRequest<CloudDictionarySnapshotRequest>(result));
      setNotice("快照已入列，将在输入法空闲时应用");
    }, "快照入列失败，本机词库未改变");
  }

  async function cancel() {
    if (busyRef.current) return;
    const lifecycle = lifecycleRevision.current;
    const confirmed = await confirm({
      title: "取消待应用快照",
      message: "待应用的云词库快照会被撤销。",
      confirmLabel: "取消快照",
      cancelLabel: "保留",
    });
    if (!confirmed || busyRef.current || lifecycle !== lifecycleRevision.current) return;
    void run(async (revision, currentLifecycle) => {
      const result = await client.request({ operation: "snapshot_cancel" });
      if (currentLifecycle !== lifecycleRevision.current || revision !== statusRevision.current)
        return;
      setRequest(cloudResponseRequest<CloudDictionarySnapshotRequest>(result));
      setNotice("已取消待应用快照");
    }, "取消快照失败");
  }

  useEffect(() => {
    const lifecycle = ++lifecycleRevision.current;
    statusRevision.current++;
    busyRef.current = false;
    setBusy(false);
    setPreview(null);
    setPreviewToken(null);
    void refreshStatus();
    const timer = window.setInterval(() => {
      void refreshStatus();
    }, 2000);
    return () => {
      if (lifecycle === lifecycleRevision.current) lifecycleRevision.current++;
      statusRevision.current++;
      busyRef.current = false;
      window.clearInterval(timer);
    };
  }, [client]);

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
            <button
              type="button"
              className="secondary"
              onClick={() => {
                setPreview(null);
                setPreviewToken(null);
              }}
              disabled={busy}
            >
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
  const { confirm, confirmation } = useConfirm();
  const [kind, setKind] = useState<CloudDictionaryKind>("pinyin");
  const [code, setCode] = useState("");
  const [scheme, setScheme] = useState("pinyin");
  const [profile, setProfile] = useState("xiaohe");
  const [confirmed, setConfirmed] = useState<{
    code: string;
    scheme: string;
    profile: string;
  } | null>(null);
  const [entries, setEntries] = useState<CloudDictionaryCatalogEntry[]>([]);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [revision, setRevision] = useState(0);
  const [normalized, setNormalized] = useState("");
  const [form, setForm] = useState<{
    entry: CloudDictionaryCatalogEntry;
    replacement: boolean;
    code: string;
    word: string;
    weight: number;
  } | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("查询基础词库与当前账号的完整目录");
  const requestRevision = useRef(0);
  const busyRef = useRef(false);

  useEffect(() => {
    requestRevision.current++;
    busyRef.current = false;
    setBusy(false);
    setEntries([]);
    setForm(null);
    setConfirmed(null);
    setHasMore(false);
    setOffset(0);
    setRevision(0);
    setNormalized("");
    return () => {
      requestRevision.current++;
      busyRef.current = false;
    };
  }, [client]);

  async function run(action: (requestRevision: number) => Promise<void>, failure: string) {
    if (busyRef.current) return;
    const current = ++requestRevision.current;
    busyRef.current = true;
    setBusy(true);
    try {
      await action(current);
    } catch {
      if (current === requestRevision.current) setNotice(failure);
    } finally {
      if (current === requestRevision.current) {
        busyRef.current = false;
        setBusy(false);
      }
    }
  }

  async function load(
    current: number,
    nextOffset: number,
    query: { code: string; scheme: string; profile: string },
  ) {
    let result;
    try {
      result = await client.request({
        operation: "catalog",
        kind,
        code: query.code,
        offset: nextOffset,
        scheme: query.scheme,
        profile: query.profile,
      });
    } catch (error) {
      if (current === requestRevision.current) {
        setEntries([]);
        setForm(null);
        setConfirmed(null);
        setHasMore(false);
        setOffset(0);
        setRevision(0);
        setNormalized("");
      }
      throw error;
    }
    if (current !== requestRevision.current) return;
    setEntries(cloudDictionaryCatalogEntries<CloudDictionaryCatalogEntry>(result));
    setOffset(typeof result.offset === "number" ? result.offset : nextOffset);
    setHasMore(result.has_more === true);
    setRevision(typeof result.revision === "number" ? result.revision : 0);
    setNormalized(typeof result.normalized === "string" ? result.normalized : query.code);
    setConfirmed(query);
  }

  function queryCatalog(
    nextOffset = 0,
    queryOverride?: { code: string; scheme: string; profile: string },
  ) {
    const query = queryOverride ?? { code: code.trim(), scheme, profile };
    if (kind !== "quick" && !query.code) {
      setNotice("请输入查询编码");
      return;
    }
    return run(async (current) => {
      await load(current, nextOffset, query);
      if (current === requestRevision.current) setNotice("完整目录已刷新");
    }, "无法访问完整云词库目录，请确认账号已登录");
  }

  function beginEdit(entry: CloudDictionaryCatalogEntry) {
    if (!busyRef.current)
      setForm({
        entry,
        replacement: true,
        code: entry.code,
        word: entry.word,
        weight: entry.weight,
      });
  }

  async function reload(current: number, message: string) {
    if (!confirmed) return;
    try {
      await load(current, 0, confirmed);
      if (current === requestRevision.current) setNotice(message);
    } catch {
      if (current === requestRevision.current) setNotice(`${message}，但目录刷新失败，请重新查询`);
    }
  }

  function save() {
    if (!form) return;
    if (!form.code.trim() || !form.word.trim()) {
      setNotice("编码和词条不能为空");
      return;
    }
    return run(async (current) => {
      await client.request({
        operation: "edit_catalog",
        kind,
        code: form.entry.code,
        word: form.entry.word,
        revision,
        replacement: form.replacement
          ? { code: form.code.trim(), word: form.word, weight: form.weight }
          : null,
      });
      if (current !== requestRevision.current) return;
      setForm(null);
      await reload(current, "完整目录已保存");
    }, "目录保存失败，请刷新后重试");
  }

  function remove(entry: CloudDictionaryCatalogEntry) {
    return run(async (current) => {
      await client.request({
        operation: "edit_catalog",
        kind,
        code: entry.code,
        word: entry.word,
        revision,
        replacement: null,
      });
      if (current !== requestRevision.current) return;
      setForm((currentForm) =>
        currentForm?.entry.code === entry.code && currentForm.entry.word === entry.word
          ? null
          : currentForm,
      );
      await reload(current, "目录词条已删除");
    }, "目录删除失败，请刷新后重试");
  }

  async function confirmRemove(entry: CloudDictionaryCatalogEntry) {
    if (busyRef.current) return;
    const confirmed = await confirm({
      title: "删除目录词条",
      message: `完整目录词条“${entry.word}”只会从云端删除，本机词库不会改变。`,
      confirmLabel: "删除",
      danger: true,
    });
    if (!confirmed || busyRef.current) return;
    void remove(entry);
  }

  function changeKind(next: CloudDictionaryKind) {
    if (busyRef.current || next === kind) return;
    setKind(next);
    setEntries([]);
    setConfirmed(null);
    setForm(null);
    setOffset(0);
    setHasMore(false);
  }

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
        <div className={cloud.dictionaryToolbar}>
          <label className={`${cloud.dictionaryField} ${cloud.dictionaryDesktopOnlyField}`}>
            词库
            <select
              className={cloud.dictionaryInput}
              aria-label="词库类型"
              value={kind}
              onChange={(event) => changeKind(event.target.value as CloudDictionaryKind)}
              disabled={busy}
            >
              {cloudDictionaryKinds.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label className={cloud.dictionarySearch}>
            编码
            <input
              className={cloud.dictionaryInput}
              aria-label="完整目录编码"
              value={code}
              onChange={(event) => setCode(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void queryCatalog();
              }}
              placeholder={kind === "quick" ? "可留空" : "例如 shi、a 或 hello"}
            />
          </label>
          <button
            className={cloud.dictionaryButton}
            type="button"
            onClick={() => void queryCatalog()}
            disabled={busy || (kind !== "quick" && !code.trim())}
          >
            查询完整目录
          </button>
        </div>
        {kind === "pinyin" && (
          <div className={cloud.dictionaryActions}>
            <label className={cloud.dictionaryField}>
              编码方案
              <select
                className={cloud.dictionaryInput}
                aria-label="编码方案"
                value={scheme}
                onChange={(event) => setScheme(event.target.value)}
                disabled={busy}
              >
                <option value="pinyin">全拼</option>
                <option value="shuangpin">双拼</option>
              </select>
            </label>
            {scheme === "shuangpin" && (
              <label className={cloud.dictionaryField}>
                双拼方案
                <select
                  className={cloud.dictionaryInput}
                  aria-label="双拼方案"
                  value={profile}
                  onChange={(event) => setProfile(event.target.value)}
                  disabled={busy}
                >
                  <option value="xiaohe">小鹤</option>
                  <option value="ziranma">自然码</option>
                  <option value="microsoft">微软</option>
                  <option value="shoudao">首道</option>
                </select>
              </label>
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
                <article
                  className={cloud.dictionaryItem}
                  key={`${entry.kind}:${entry.code}:${entry.word}`}
                >
                  <button
                    type="button"
                    className={cloud.dictionaryItemMain}
                    aria-label={`编辑完整目录词条 ${entry.word}`}
                    onClick={() => beginEdit(entry)}
                    disabled={busy}
                  >
                    <strong>{entry.word}</strong>
                    <small>
                      {entry.code} · 权重 {entry.weight}
                    </small>
                  </button>
                  <div className={cloud.dictionaryItemActions}>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => beginEdit(entry)}
                      disabled={busy}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => confirmRemove(entry)}
                      disabled={busy}
                    >
                      删除
                    </button>
                  </div>
                </article>
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
            onChange={(patch) => setForm((current) => (current ? { ...current, ...patch } : null))}
            onSave={() => void save()}
            onCancel={() => setForm(null)}
          />
        )}
        {confirmed && (
          <div className={cloud.dictionaryPagination}>
            <button
              className={cloud.dictionaryButton}
              type="button"
              onClick={() => void queryCatalog(Math.max(0, offset - 100), confirmed)}
              disabled={busy || offset === 0}
            >
              上一页
            </button>
            <span>第 {Math.floor(offset / 100) + 1} 页</span>
            <button
              className={cloud.dictionaryButton}
              type="button"
              onClick={() => void queryCatalog(offset + 100, confirmed)}
              disabled={busy || !hasMore}
            >
              下一页
            </button>
          </div>
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
  const { confirm, confirmation } = useConfirm();
  const [kind, setKind] = useState<CloudDictionaryKind>("pinyin");
  const [text, setText] = useState("");
  const [scheme, setScheme] = useState("pinyin");
  const [profile, setProfile] = useState("xiaohe");
  const [jianpin, setJianpin] = useState(false);
  const [mode, setMode] = useState<CloudRankingMode>("pin");
  const [step, setStep] = useState(1);
  const [trigger, setTrigger] = useState(1);
  const [forceTop, setForceTop] = useState(false);
  const [position, setPosition] = useState(1);
  const [query, setQuery] = useState<{
    text: string;
    kind: CloudCandidateKind;
    scheme: string;
    profile: string;
    limit: number;
  } | null>(null);
  const [candidates, setCandidates] = useState<CloudCandidate[]>([]);
  const [positions, setPositions] = useState<CloudFixedPosition[]>([]);
  const [context, setContext] = useState("");
  const [revision, setRevision] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("仅在点击查询时发送编码；修改只保存到当前账号");
  const textRef = useRef("");
  const requestRevision = useRef(0);
  const busyRef = useRef(false);

  useEffect(() => {
    requestRevision.current++;
    busyRef.current = false;
    setBusy(false);
    setCandidates([]);
    setPositions([]);
    setQuery(null);
    setContext("");
    setRevision(0);
    return () => {
      requestRevision.current++;
      busyRef.current = false;
    };
  }, [client]);

  async function run(action: (requestRevision: number) => Promise<void>, failure: string) {
    if (busyRef.current) return;
    const current = ++requestRevision.current;
    busyRef.current = true;
    setBusy(true);
    try {
      await action(current);
    } catch {
      if (current === requestRevision.current) setNotice(failure);
    } finally {
      if (current === requestRevision.current) {
        busyRef.current = false;
        setBusy(false);
      }
    }
  }

  async function load(
    current: number,
    nextQuery: {
      text: string;
      kind: CloudCandidateKind;
      scheme: string;
      profile: string;
      limit: number;
    },
  ) {
    let result;
    try {
      result = await client.request({ operation: "candidates", ...nextQuery });
    } catch (error) {
      if (current === requestRevision.current) {
        setCandidates([]);
        setPositions([]);
        setQuery(null);
        setContext("");
        setRevision(0);
      }
      throw error;
    }
    if (current !== requestRevision.current) return;
    const nextCandidates = Array.isArray(result.candidates)
      ? result.candidates.filter(
          (candidate) =>
            candidate && typeof candidate.code === "string" && typeof candidate.word === "string",
        )
      : [];
    // Publish the candidate page as soon as it arrives. Loading fixed
    // positions is a secondary request and must not delay the visible result
    // (or make a fast query appear to have timed out on touch hosts).
    setCandidates(nextCandidates);
    setContext(typeof result.context === "string" ? result.context : "");
    setRevision(typeof result.revision === "number" ? result.revision : 0);
    setPositions([]);
    setQuery(nextQuery);
    if (nextQuery.kind !== "quick" && typeof result.context === "string" && result.context) {
      const fixed = await client.request({
        operation: "fixed_positions",
        context: result.context,
        offset: 0,
      });
      if (current !== requestRevision.current) return;
      const nextPositions = Array.isArray(fixed.positions)
        ? fixed.positions.filter(
            (item) => item && typeof item.code === "string" && typeof item.word === "string",
          )
        : [];
      setPositions(nextPositions);
    }
  }

  function queryCandidates() {
    const value = textRef.current.trim();
    if (!value) {
      setNotice("请输入编码");
      return;
    }
    const nextQuery = {
      text: value,
      kind: jianpin && kind === "pinyin" ? ("jianpin" as const) : kind,
      scheme,
      profile,
      limit: 100,
    };
    return run(async (current) => {
      await load(current, nextQuery);
      if (current === requestRevision.current) setNotice("云端候选已刷新");
    }, "无法查询云端候选，请确认账号已登录");
  }

  async function reload(current: number, message: string) {
    if (!query) return;
    try {
      await load(current, query);
      if (current === requestRevision.current) setNotice(message);
    } catch {
      if (current === requestRevision.current) setNotice(`${message}，但候选刷新失败，请重新查询`);
    }
  }

  async function rank(candidate: CloudCandidate) {
    if (!query) return;
    if (!(await confirm({ title: "调整排序", message: "调整此云端候选的排序。" }))) return;
    return run(async (current) => {
      const result = await client.request({
        operation: "rank",
        ...query,
        code: candidateMutationCode(candidate),
        word: candidate.word,
        revision,
        mode,
        linear_step: step,
        trigger_count: trigger,
        force_top: forceTop,
      });
      await reload(
        current,
        result.changed === true
          ? "云端排序已更新"
          : `已记录本次选择，本次未调整排序；累计 ${result.selection_count ?? 0} 次`,
      );
    }, "云端调频失败，请刷新后重试");
  }

  async function remove(candidate: CloudCandidate) {
    if (!query || (kind !== "english" && Array.from(candidate.word).length <= 1)) return;
    const confirmed = await confirm({
      title: "删除云端候选",
      message: `“${candidate.word}”会从云端候选中删除。`,
      confirmLabel: "删除",
      danger: true,
    });
    if (!confirmed) return;
    return run(async (current) => {
      await client.request({
        operation: "remove_candidate",
        ...query,
        code: candidateMutationCode(candidate),
        word: candidate.word,
        revision,
      });
      await reload(current, "云端候选已删除");
    }, "删除云端候选失败，请刷新后重试");
  }

  async function setFixed(candidate: CloudCandidate, nextPosition: number | null) {
    if (!query || !context) return;
    const confirmed = await confirm(
      nextPosition === null
        ? { title: "取消固定位置", message: `“${candidate.word}”将不再固定在某一位。` }
        : { title: "固定位置", message: `“${candidate.word}”将固定到第 ${nextPosition} 位。` },
    );
    if (!confirmed) return;
    return run(async (current) => {
      await client.request({
        operation: "set_fixed_position",
        context,
        code: candidateMutationCode(candidate),
        word: candidate.word,
        position: nextPosition,
        revision,
      });
      await reload(current, nextPosition === null ? "固定位置已取消" : "固定位置已更新");
    }, "固定位置更新失败，请刷新后重试");
  }

  async function unfix(item: CloudFixedPosition) {
    if (!query) return;
    const confirmed = await confirm({
      title: "取消固定位置",
      message: `“${item.word}”将不再固定在某一位。`,
    });
    if (!confirmed) return;
    return run(async (current) => {
      await client.request({
        operation: "set_fixed_position",
        context: item.context,
        code: item.code,
        word: item.word,
        position: null,
        revision,
      });
      await reload(current, "固定位置已取消");
    }, "固定位置更新失败，请刷新后重试");
  }

  function changeKind(next: CloudDictionaryKind) {
    if (busyRef.current || next === kind) return;
    setKind(next);
    setJianpin(false);
    setCandidates([]);
    setPositions([]);
    setQuery(null);
    setContext("");
  }

  return (
    <main className={`native-panel ${cloud.dictionaryPanel}`} aria-label="云端候选排序">
      {confirmation}
      <CloudPanelHeader
        title="云端候选排序"
        onBack={() => void (client.back ? client.back() : client.close())}
        onClose={() => void client.close()}
      />
      <div className={cloud.dictionaryBody}>
        <div className={cloud.dictionaryToolbar}>
          <label className={`${cloud.dictionaryField} ${cloud.dictionaryDesktopOnlyField}`}>
            词库
            <select
              className={cloud.dictionaryInput}
              aria-label="词库类型"
              value={kind}
              onChange={(event) => changeKind(event.target.value as CloudDictionaryKind)}
              disabled={busy}
            >
              {cloudDictionaryKinds.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label className={cloud.dictionarySearch}>
            编码
            <input
              aria-label="云端候选编码"
              value={text}
              onChange={(event) => {
                textRef.current = event.target.value;
                setText(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") void queryCandidates();
              }}
            />
          </label>
          <button
            type="button"
            onClick={() => void queryCandidates()}
            disabled={busy || !text.trim()}
          >
            查询云端候选
          </button>
        </div>
        {kind === "pinyin" && (
          <div className={cloud.dictionaryActions}>
            <label>
              <input
                type="checkbox"
                checked={jianpin}
                onChange={(event) => setJianpin(event.target.checked)}
                disabled={busy}
              />
              简拼候选
            </label>
            <label>
              编码方案
              <select
                aria-label="候选编码方案"
                value={scheme}
                onChange={(event) => setScheme(event.target.value)}
                disabled={busy}
              >
                <option value="pinyin">全拼</option>
                <option value="shuangpin">双拼</option>
              </select>
            </label>
            {scheme === "shuangpin" && (
              <label>
                双拼方案
                <select
                  aria-label="候选双拼方案"
                  value={profile}
                  onChange={(event) => setProfile(event.target.value)}
                  disabled={busy}
                >
                  <option value="xiaohe">小鹤</option>
                  <option value="ziranma">自然码</option>
                  <option value="microsoft">微软</option>
                  <option value="shoudao">首道</option>
                </select>
              </label>
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
              <input
                type="checkbox"
                checked={forceTop}
                onChange={(event) => setForceTop(event.target.checked)}
                disabled={busy}
              />
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
                <article
                  className={cloud.dictionaryItem}
                  key={`${candidateMutationCode(candidate)}:${candidate.word}`}
                >
                  <button
                    type="button"
                    className={cloud.dictionaryItemMain}
                    aria-label={`调频候选 ${candidate.word}`}
                    onClick={() => void rank(candidate)}
                    disabled={busy || kind === "quick"}
                  >
                    <strong>
                      {index + 1}. <span className="break-anywhere">{candidate.word}</span>
                    </strong>
                    <small>
                      {candidate.code} · 权重 {candidate.weight}
                    </small>
                  </button>
                  {kind !== "quick" && (
                    <div className={cloud.dictionaryItemActions}>
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
                    </div>
                  )}
                </article>
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

type EmojiPage = "home" | "emoji" | "sticker" | "gif" | "kaomoji" | "symbols" | "clipboard";

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
const panelSurface = "bg-[#202027] group-data-[panel-theme=light]:bg-[#f7f8fa]";
const panelText = "text-[#f5f5f7] group-data-[panel-theme=light]:text-[#202124]";
const panelMuted = "text-[#aeb0b7] group-data-[panel-theme=light]:text-[#656a73]";
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
const panelHeading = `m-0 text-sm font-semibold ${panelText}`;
const panelContent =
  "min-h-0 flex-1 overflow-y-auto px-6 pt-2 pb-[52px] max-phone:px-3.5 [scrollbar-color:#77747d_transparent]";
const panelChip = (chosen: boolean) =>
  `shrink-0 rounded-[7px] border px-2.5 py-1.5 font-[inherit] text-xs ${
    chosen
      ? `${panelAccentBorder} ${panelChosen}`
      : `border-transparent bg-transparent ${panelDim} ${panelRaise} focus-visible:bg-[#303038] focus-visible:text-[#f5f5f7] group-data-[panel-theme=light]:focus-visible:bg-[#eceef3] group-data-[panel-theme=light]:focus-visible:text-[#202124]`
  }`;
const panelEmpty = `m-0 flex min-h-[220px] items-center justify-center text-center text-sm ${panelMuted}`;
/*
 * A tile. Grid tiles are uniform squares; flow tiles hold kaomoji and captioned symbols, so they size
 * to their content and wrap their text instead. The `:nth-child(n)` the stylesheet used on the group
 * was only there to win a specificity fight with the grid rule -- a fight utilities do not have.
 */
const panelTile = (flow: boolean) =>
  `flex min-w-0 items-center justify-center overflow-hidden rounded-[9px] border border-transparent bg-transparent text-center font-emoji-text leading-[1.2] break-anywhere ${panelText} hover:border-[#555560] hover:bg-[#303038] focus-visible:border-[#555560] focus-visible:bg-[#303038] group-data-[panel-theme=light]:hover:border-[#c8ccd5] group-data-[panel-theme=light]:hover:bg-[#eceef3] group-data-[panel-theme=light]:focus-visible:border-[#c8ccd5] group-data-[panel-theme=light]:focus-visible:bg-[#eceef3] disabled:cursor-wait disabled:opacity-55 ${
    flow
      ? "max-w-full flex-[0_1_auto] min-h-12 px-3 py-2.5 text-xl whitespace-pre-wrap"
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
  const [page, setPage] = useState<EmojiPage>(initialPage);
  const [itemPage, setItemPage] = useState(0);
  const itemsPerPage = 48;
  const [query, setQuery] = useState("");
  const [categories, setCategories] = useState({ emoji: "all", symbols: "all" });
  const [recent, setRecent] = useState<EmojiCatalogItem[]>(() => {
    try {
      const value: unknown =
        typeof window === "undefined"
          ? null
          : JSON.parse(window.localStorage.getItem("msime.emoji.recent") ?? "null");
      return Array.isArray(value)
        ? (value
            .filter(
              (item) => item && typeof item.text === "string" && typeof item.keywords === "string",
            )
            .slice(0, 28) as EmojiCatalogItem[])
        : [];
    } catch {
      return [];
    }
  });
  const [clipboard, setClipboard] = useState<string[]>([]);
  const [clipboardBusy, setClipboardBusy] = useState(false);
  const [clipboardLoadFailed, setClipboardLoadFailed] = useState(false);
  const [clipboardRefresh, setClipboardRefresh] = useState(0);
  const [activationMode, setActivationMode] = useState<"copy" | "input">("copy");
  const operationRevision = useRef(0);
  const [clipboardEnabled, setClipboardEnabled] = useState<boolean | null>(null);
  const clipboardMutation = useRef(false);
  const clipboardGeneration = useRef(0);
  const deletedRowFocus = useRef<{ element: HTMLElement; index: number; query: string } | null>(
    null,
  );
  const [notice, setNoticeText] = useState("");
  const noticeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
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

  function setNotice(message: string, temporary = false) {
    if (noticeTimer.current !== null) clearTimeout(noticeTimer.current);
    noticeTimer.current = null;
    setNoticeText(message);
    if (temporary) {
      noticeTimer.current = setTimeout(() => {
        noticeTimer.current = null;
        setNoticeText("");
      }, 1600);
    }
  }

  useEffect(() => {
    clipboardMutation.current = false;
    deletedRowFocus.current = null;
    setClipboardBusy(false);
    setNotice("");
    return () => {
      operationRevision.current++;
      if (noticeTimer.current !== null) clearTimeout(noticeTimer.current);
      noticeTimer.current = null;
    };
  }, [client]);

  async function runOperation(action: (revision: number) => Promise<void>, failure: string) {
    if (clipboardMutation.current) return;
    const revision = ++operationRevision.current;
    clipboardMutation.current = true;
    setClipboardBusy(true);
    try {
      await action(revision);
    } catch {
      if (revision === operationRevision.current) setNotice(failure);
    } finally {
      if (revision === operationRevision.current) {
        clipboardMutation.current = false;
        setClipboardBusy(false);
      }
    }
  }

  useEffect(() => {
    try {
      window.localStorage.setItem("msime.emoji.recent", JSON.stringify(recent.slice(0, 28)));
    } catch {
      /* preference is optional */
    }
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
    let active = true;
    setCatalogLoading(true);
    void Promise.resolve()
      .then(() => client.loadCatalog!())
      .then((next) => {
        if (!active) return;
        const unavailable = next.unavailable ?? [];
        setCatalog((current) => ({
          emoji: unavailable.includes("emoji") ? current.emoji : next.emoji,
          kaomoji: unavailable.includes("kaomoji") ? current.kaomoji : next.kaomoji,
          symbols: unavailable.includes("symbols") ? current.symbols : next.symbols,
        }));
        setCatalogUnavailable(unavailable);
      })
      .catch(() => {
        if (active) setCatalogUnavailable(["emoji", "kaomoji", "symbols"]);
      })
      .finally(() => {
        if (active) setCatalogLoading(false);
      });
    return () => {
      active = false;
    };
  }, [client, catalogRetry]);

  useEffect(() => {
    if (!client.clipboard?.list) {
      setClipboardLoadFailed(false);
      return;
    }
    let active = true;
    let unsubscribe: (() => void) | undefined;
    let poll: ReturnType<typeof setInterval> | undefined;
    let inFlight = 0;
    let refreshPending = false;
    const refresh = () => {
      if (!active) return;
      if (inFlight !== 0) {
        refreshPending = true;
        // A newer notification invalidates the current snapshot, but only
        // one follow-up read is needed even when many notifications arrive.
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
          if (active && request === clipboardGeneration.current) {
            setClipboard(enabled ? value : []);
            setClipboardEnabled(enabled);
            setClipboardLoadFailed(false);
          }
        })
        .catch(() => {
          if (active && request === clipboardGeneration.current) {
            setClipboard([]);
            setClipboardEnabled(null);
            setClipboardLoadFailed(true);
          }
        })
        .finally(() => {
          inFlight--;
          if (active && refreshPending) {
            refreshPending = false;
            refresh();
          }
        });
    };
    const pollVisible = () => {
      if (
        active &&
        document.visibilityState === "visible" &&
        !clipboardMutation.current &&
        inFlight === 0
      )
        refresh();
    };
    const start = async () => {
      try {
        const stop = await client.clipboard?.onChanged?.(refresh);
        if (!active) {
          stop?.();
          return;
        }
        unsubscribe = stop;
      } catch {
        /* Keep polling if host notification registration fails. */
      }
      if (!active) return;
      // Close the gap between the initial snapshot and subscription setup.
      refresh();
      if (unsubscribe && poll !== undefined) {
        clearInterval(poll);
        poll = undefined;
        document.removeEventListener("visibilitychange", pollVisible);
      }
    };
    // Listing must not wait for a slow or stalled notification registration.
    // Use Windows' 400 ms interval until the host subscription is ready.
    refresh();
    if (page === "clipboard") {
      poll = setInterval(pollVisible, 400);
      document.addEventListener("visibilitychange", pollVisible);
    }
    void start();
    return () => {
      active = false;
      refreshPending = false;
      ++clipboardGeneration.current;
      if (poll !== undefined) clearInterval(poll);
      document.removeEventListener("visibilitychange", pollVisible);
      unsubscribe?.();
    };
  }, [client, page, clipboardRefresh]);

  type DisplayGroup = EmojiCatalogGroup & { moreTarget?: EmojiPage; flow?: boolean };
  const groups =
    page === "emoji" ? catalog.emoji : page === "kaomoji" ? catalog.kaomoji : catalog.symbols;
  const categoryPage = page === "emoji" || page === "symbols" ? page : null;
  const categoryKey = (group: EmojiCatalogGroup) =>
    page === "symbols" && group.parent ? `parent:${group.parent}` : `group:${group.title}`;
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
  const recentGroup = { title: "最近使用", icon: "◷", items: recent };
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
      items: group.items.filter((item) => matchesEmojiItem(item, query)),
    }))
    .filter((group) => group.items.length);
  const symbolPreview = query
    ? flattenGroups(catalog.symbols)
        .filter((item) => matchesEmojiItem(item, query))
        .slice(0, 18)
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
      items: flattenGroups(catalog.kaomoji)
        .filter((item) => matchesEmojiItem(item, query))
        .slice(0, 15),
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
      event.nativeEvent.isComposing ||
      event.keyCode === 229 ||
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
  return (
    <main
      {...navigation}
      className={`native-panel group flex min-h-screen flex-col overflow-hidden ${panelSurface} ${panelText}`}
      data-panel-theme={theme}
      aria-label="表情与符号"
    >
      <header
        className={`native-panel-header flex-[0_0_38px] border-b ${panelDivider} ${panelSurface} ${panelText} [&>button]:text-[#aeb0b7] group-data-[panel-theme=light]:[&>button]:text-[#656a73]`}
      >
        <span>Emoji and more</span>
        <button
          type="button"
          aria-label="关闭"
          disabled={clipboardBusy}
          onClick={() => void closeEmoji()}
        >
          ×
        </button>
      </header>
      <div
        className={`mx-6 mt-3.5 flex flex-[0_0_52px] items-center gap-2 rounded-[10px] px-3.5 max-phone:mx-3.5 ${panelField} focus-within:border-[#5fbf84] focus-within:shadow-[0_0_0_1px_rgba(95,191,132,0.35)] group-data-[panel-theme=light]:focus-within:border-[#2c7a4b] group-data-[panel-theme=light]:focus-within:shadow-[0_0_0_1px_rgba(154,98,173,0.28)]`}
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
          onChange={(event) => {
            setQuery(event.target.value);
            setItemPage(0);
            setNotice("");
          }}
          placeholder={
            page === "clipboard"
              ? "搜索剪贴板"
              : page === "emoji"
                ? "Search emojis"
                : page === "kaomoji"
                  ? "Search kaomoji"
                  : page === "symbols"
                    ? "Search symbols"
                    : page === "home"
                      ? "Search emoji, kaomoji, and symbols"
                      : "Search"
          }
        />
      </div>
      <nav
        className="flex flex-[0_0_72px] items-stretch gap-1 px-[18px] pt-2 max-phone:px-2"
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
            <span className="min-h-7 font-emoji text-[23px] leading-[1.1]" aria-hidden="true">
              {item.icon}
            </span>
            <small className="m-0 text-[10px] leading-[1.2] text-inherit max-phone:text-[9px]">
              {item.label}
            </small>
          </button>
        ))}
      </nav>
      {isDetail && (
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
              onClick={() => {
                setCatalogLoading(true);
                setCatalogRetry((value) => value + 1);
              }}
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
            <button
              type="button"
              className={`rounded-md border px-[9px] py-[5px] font-[inherit] disabled:cursor-wait disabled:opacity-55 ${
                effectiveMode === "copy"
                  ? `${panelAccentBorder} ${panelChosen}`
                  : "border-transparent bg-transparent text-inherit"
              }`}
              aria-pressed={effectiveMode === "copy"}
              disabled={clipboardBusy}
              onClick={() => {
                setActivationMode("copy");
                setNotice("点击项目即可复制");
              }}
            >
              复制
            </button>
            <button
              type="button"
              className={`rounded-md border px-[9px] py-[5px] font-[inherit] disabled:cursor-wait disabled:opacity-55 ${
                effectiveMode === "input"
                  ? `${panelAccentBorder} ${panelChosen}`
                  : "border-transparent bg-transparent text-inherit"
              }`}
              aria-pressed={effectiveMode === "input"}
              disabled={clipboardBusy}
              onClick={() => {
                setActivationMode("input");
                setNotice("点击项目即可输入到原应用");
              }}
            >
              输入到原应用
            </button>
          </div>
        )}
      {categoryPage && (
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
      {page === "clipboard" ? (
        <section className={`${panelContent} pt-3.5`} aria-label="剪贴板历史">
          <div className={panelToolbar}>
            <h2 className={panelHeading}>剪贴板</h2>
            <div className="ml-auto flex gap-2">
              {client.clipboard?.list && (
                <button
                  type="button"
                  className={panelAction}
                  disabled={clipboardBusy}
                  onClick={() => {
                    if (!clipboardMutation.current) setClipboardRefresh((value) => value + 1);
                  }}
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
            <div className="mb-[22px]" key={JSON.stringify([group.parent, group.title])}>
              <div className="mb-2 flex min-h-[34px] items-center gap-[9px]">
                <span className="w-6 text-center font-emoji text-xl">{group.icon}</span>
                <h2 className={panelHeading}>{group.title}</h2>
                {group.moreTarget && (
                  <button
                    type="button"
                    className={`${panelAction} ml-auto`}
                    onClick={() => {
                      setCategories((current) => ({ ...current, emoji: "all", symbols: "all" }));
                      selectPage(group.moreTarget!);
                    }}
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
                onClick={() => setItemPage((value) => Math.max(0, value - 1))}
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
                onClick={() => setItemPage((value) => Math.min(itemPageCount - 1, value + 1))}
              >
                下一页
              </button>
            </div>
          )}
        </section>
      )}
      <p
        className={`m-0 min-h-[30px] flex-[0_0_30px] border-t px-6 py-1.5 text-center text-xs ${panelDivider} ${panelMuted}`}
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
