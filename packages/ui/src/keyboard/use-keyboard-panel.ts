import { useEffect, useRef, useState, type PointerEvent } from "react";
import { subscribeToWindowBlur } from "../core/window-focus";
import { readOptionalStorage, writeOptionalStorage } from "../core/optional-storage";
import { useLatestRef } from "../core/use-latest-ref";
import type { NineKeyShuangpinKeyFace } from "../settings/nine-key-shuangpin-profile";
import type { KeyboardInputRequest, PanelClient } from "./panel-client";
import {
  isImeCommitKey,
  isKeyboardLetter,
  keyboardKeyLabel,
  modifierPrefix,
  type KeyboardKey,
  type Modifier,
} from "./keyboard-input";
import { keyboardDeliveryNotice, MACOS_KEYBOARD_GUIDANCE } from "./keyboard-delivery-notice";
import {
  keyboardRows,
  nineKeyRows,
  nineKeyShuangpinRows,
  eighteenKeyRows,
} from "./panel-keyboard-layouts";
import { usePanelDrag } from "./use-panel-drag";

export function useKeyboardPanel({
  client,
  platform,
  layout = "twenty_six_key",
  nineKeyShuangpinKeys,
}: {
  client: PanelClient;
  platform?: string;
  layout?: "twenty_six_key" | "nine_key" | "nine_key_shuangpin";
  nineKeyShuangpinKeys?: readonly NineKeyShuangpinKeyFace[];
}) {
  const [activeLayout, setActiveLayout] = useState<
    "twenty_six_key" | "nine_key" | "nine_key_shuangpin" | "eighteen_key"
  >(() => {
    const saved = readOptionalStorage("msime.keyboard.layout");
    if (platform === "windows" && saved === "eighteen_key") return saved;
    return saved === "nine_key" || saved === "twenty_six_key" || saved === "nine_key_shuangpin"
      ? saved
      : layout;
  });
  function switchLayout() {
    const next =
      platform === "windows" && activeLayout === "twenty_six_key"
        ? "eighteen_key"
        : activeLayout === "eighteen_key"
          ? layout === "nine_key_shuangpin"
            ? "nine_key_shuangpin"
            : "nine_key"
          : activeLayout === "twenty_six_key"
            ? layout === "nine_key_shuangpin"
              ? "nine_key_shuangpin"
              : "nine_key"
            : "twenty_six_key";
    setActiveLayout(next);
    modifiersRef.current = new Set();
    setActiveModifiers(new Set());
    writeOptionalStorage("msime.keyboard.layout", next);
  }
  const previousHostLayout = useRef(layout);
  useEffect(() => {
    // 首次挂载（包括 StrictMode 重放）保留已存储的布局偏好。
    if (previousHostLayout.current === layout) return;
    previousHostLayout.current = layout;
    setActiveLayout(layout);
  }, [layout]);
  const sourceRows =
    activeLayout === "eighteen_key"
      ? eighteenKeyRows
      : activeLayout === "nine_key_shuangpin"
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
      : platform === "linux"
        ? // Linux 把 Win 键叫 Super（打字统计的热力图也这样叫），键位和发送的按键不变。
          sourceRows.map((row) =>
            row.map((item) => (item.label === "Win" ? { ...item, label: "Super" } : item)),
          )
        : sourceRows;
  const [, setActiveModifiers] = useState<Set<Modifier>>(new Set());
  const modifiersRef = useRef<Set<Modifier>>(new Set());
  // macOS 和 Linux 的屏幕键盘都不抢焦点，按键发给用户刚点过的输入框，标题栏先告诉用户这一点。
  const guided = platform === "macos" || platform === "linux";
  const [notice, setNotice] = useState(guided ? MACOS_KEYBOARD_GUIDANCE : "Touch keyboard");
  const platformRef = useLatestRef(platform);
  useEffect(() => {
    if (guided)
      setNotice((shown) => (shown === "Touch keyboard" ? MACOS_KEYBOARD_GUIDANCE : shown));
  }, [guided]);
  const [openingVoice, setOpeningVoice] = useState(false);
  type QueuedKey = { request: KeyboardInputRequest; description: string };
  const inputQueue = useRef<{
    active: boolean;
    running: boolean;
    openingVoice: boolean;
    pending: QueuedKey[];
  }>({ active: true, running: false, openingVoice: false, pending: [] });
  const keyRepeat = useRef<{ delay?: number; interval?: number }>({});
  // Windows 与原生 KeyboardPanel::OnMouseUp 一致：记住按下的键，仅在同一键上松开时发送，滑到别处松开则取消，长按不连发。
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
    // React 可能把 Shift 和随后按键的点击合批处理；同步更新可见键面，保持两个事件之间的显示与状态一致。
    if (typeof document === "undefined") return;
    const buttons = document.querySelectorAll<HTMLButtonElement>(
      "[data-keyboard-layout-grid] .keyboard-key",
    );
    rows.flat().forEach((item, index) => {
      const button = buttons[index];
      if (!button) return;
      const shifted = modifiers.has("Shift") && item.label.length === 1;
      const face = keyboardKeyLabel(item, shifted);
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
          // 不激活窗口的面板打开期间，外部编辑器可能变化；每次点击都捕获目标，并与发送串行执行，避免后一次点击覆盖前一键的目标。
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
    const letter = isKeyboardLetter(keyToPress);
    // 与 Windows 键盘面板一致，字母的 Caps Lock 与 Shift 相互取反；标点只受 Shift 影响。
    const withShift = letter ? caps !== shift : shift;
    const modifiers = {
      ctrl: activeModifiers.has("Ctrl"),
      alt: activeModifiers.has("Alt"),
      win: activeModifiers.has("Win"),
    };
    const includeStickyModifiers = !isImeCommitKey(keyToPress.virtualKey);
    const prefix =
      platform === "linux"
        ? modifierPrefix(activeModifiers).replace("Win", "Super")
        : modifierPrefix(activeModifiers);
    const displayedLabel = keyboardKeyLabel(keyToPress, withShift);
    const description = `${prefix}${prefix ? "+" : ""}${displayedLabel}`;
    const request: KeyboardInputRequest = {
      virtual_key: keyToPress.virtualKey,
      shift: withShift && includeStickyModifiers,
      modifiers,
      include_sticky_modifiers: includeStickyModifiers,
      ...(keyToPress.groupedLetter
        ? {
            grouped_letter: keyToPress.groupedLetter,
            shift: false,
            include_sticky_modifiers: false,
          }
        : {}),
    };
    setNotice(
      client.sendKey ? `正在发送：${description}` : `已准备：${description}（等待宿主注入能力）`,
    );
    if (client.sendKey) {
      // 原生实现同步发送完整的 SendInput 序列；Tauri 命令异步执行，因此显式排队，避免后按的键越过正在发送的键或绕过有界失败队列。
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
    // Num Lock 是 Linux 扩展布局的锁定键，长按不能像普通小键盘键一样反复切换。
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
      // 触摸和笔会隐式捕获指针；释放捕获，让 pointerup 落到指针下实际的键上。
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
  return {
    activeLayout,
    rows,
    renderedModifiers: modifiersRef.current,
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
  };
}

export type KeyboardPanelState = ReturnType<typeof useKeyboardPanel>;
