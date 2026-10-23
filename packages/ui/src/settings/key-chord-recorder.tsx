import { useState } from "react";
import { SettingsShortcutKey } from "./settings-shortcut-key";
import { chordEventPreview, chordFromKeyEvent, isModifierKeyCode } from "./key-chord-event";

export interface KeyChordRecorderProps {
  /** 当前绑定的规范写法；空串表示未绑定。 */
  value: string;
  /** 录下一个组合键（或 Backspace / Delete 录下空串解绑）后回调；写法是规范形或宿主不认得的 code 串，能不能写由 `onCommit` 一侧判。 */
  onCommit: (chord: string) => void;
  disabled?: boolean;
  /** 行内错误文本由外层持有、随这次提交成败更新；控件内部只负责采集。 */
  error?: string;
}

/** 录键控件：点击进入录制态，按下的组合键实时显示，Esc 取消，Backspace / Delete 解绑。 */
export function KeyChordRecorder({ value, onCommit, disabled, error }: KeyChordRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [preview, setPreview] = useState("");
  const stop = () => {
    setRecording(false);
    setPreview("");
  };
  const onKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.code === "Escape") {
      stop();
      return;
    }
    if (event.code === "Backspace" || event.code === "Delete") {
      stop();
      onCommit("");
      return;
    }
    setPreview(chordEventPreview(event));
    if (!isModifierKeyCode(event.code)) {
      stop();
      onCommit(chordFromKeyEvent(event));
    }
  };
  return (
    <span className="flex min-w-0 flex-col items-end gap-1">
      <button
        type="button"
        className="rounded-lg border border-transparent px-2 py-1 text-[13px] text-body transition-colors hover:border-[var(--p-border)] focus-visible:border-accent focus-visible:outline-none disabled:opacity-50"
        disabled={disabled}
        aria-label={recording ? "正在录制快捷键" : "录制快捷键"}
        aria-pressed={recording}
        onClick={() => setRecording(true)}
        // 失焦取消录制：录制中点到别处等同于放弃这次录入。
        onBlur={() => recording && stop()}
        onKeyDown={onKeyDown}
        onKeyUp={(event) => recording && setPreview(chordEventPreview(event))}
      >
        {recording ? (
          preview ? (
            <SettingsShortcutKey>{preview}</SettingsShortcutKey>
          ) : (
            <span className="text-muted">按下组合键…</span>
          )
        ) : value ? (
          <SettingsShortcutKey>{value}</SettingsShortcutKey>
        ) : (
          <span className="text-muted">未绑定</span>
        )}
      </button>
      {error && !recording && <span className="text-xs text-danger">{error}</span>}
    </span>
  );
}
