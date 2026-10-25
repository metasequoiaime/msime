import { useState } from "react";
import type { ToggleHotkeyCheckRequest, ToggleHotkeyCheckResult } from "../core/settings-client";
import type { KeybindingPreferences } from "./keybinding-preferences";
import { GroupList, Row } from "../core/platform-controls";
import { SettingsRowStack } from "./settings-row-stack";
import { SettingsGroupNote } from "./settings-group-note";
import { KeyChordRecorder } from "./key-chord-recorder";

type ToggleHotkeyField = ToggleHotkeyCheckRequest["field"];

/** `checkToggleHotkey` 回传的 `owner` 到设置页文案：切换热键用对应行名，开关占用用那个开关的含义。 */
const OWNER_LABELS: Record<string, string> = {
  toggle_english_candidates: "中英切换（英文候选模式）",
  toggle_full_width: "全角 / 半角切换",
  toggle_punctuation: "中英文标点切换",
  toggle_voice_input: "语音输入开始／结束",
  switch_language_shift: "Shift+Space 中英文切换",
  switch_language_ctrl_space: "Ctrl+Space 中英文切换",
  switch_language_ctrl_alt_space: "Ctrl+Alt+Space 中英文切换",
  toggle_character_set_ctrl_shift_f: "Ctrl+Shift+F 简繁切换",
  toggle_japanese_ctrl_shift_j: "Ctrl+Shift+J 中日切换",
  toggle_fullwidth_option_shift_h: "Option+Shift+H 全角 / 半角切换",
  macos_screen_keyboard: "屏幕键盘快捷键",
  voice_hotkey: "语音输入快捷键",
  macos_maintenance: "macOS 维护快捷键",
};

const ROWS: { field: ToggleHotkeyField; title: string }[] = [
  { field: "toggle_english_candidates", title: "中英切换（英文候选模式）" },
  { field: "toggle_full_width", title: "全角 / 半角切换" },
  { field: "toggle_punctuation", title: "中英文标点切换" },
];

export interface ToggleHotkeysSectionProps {
  keybindings: KeybindingPreferences;
  /** 录键写入前的宿主试写（`SettingsClient.checkToggleHotkey`）；没有时整组不渲染，避免绕过校验写入。 */
  check?: (request: ToggleHotkeyCheckRequest) => Promise<ToggleHotkeyCheckResult>;
  /** `voice_input.hotkey_ctrl_f9`，语音热键的占用随它开合。 */
  voiceCtrlF9: boolean;
  onChange: (patch: Partial<KeybindingPreferences>) => void;
  disabled?: boolean;
  /** 语音页只编辑语音绑定，其余三个切换键仍在快捷键页。 */
  voiceOnly?: boolean;
}

function rejectionMessage(result: ToggleHotkeyCheckResult & { ok: false }): string {
  if (result.kind === "conflict") {
    const owner = OWNER_LABELS[result.owner] ?? result.owner;
    return `与「${owner}」相同，请换一个组合键`;
  }
  return "无法识别的组合键";
}

/** 共享录键组：语音页编辑语音绑定，快捷键页编辑另外三个切换键；宿主提供共享校验才挂载。 */
export function ToggleHotkeysSection({
  keybindings,
  check,
  voiceCtrlF9,
  onChange,
  disabled,
  voiceOnly = false,
}: ToggleHotkeysSectionProps) {
  const [errors, setErrors] = useState<Partial<Record<ToggleHotkeyField, string>>>({});
  if (!check) return null;
  const commit = async (field: ToggleHotkeyField, chord: string) => {
    try {
      const result = await check({
        field,
        chord,
        keybindings: { ...keybindings } as Record<string, unknown>,
        voice_ctrl_f9: voiceCtrlF9,
        physical_recording: true,
      });
      if (result.ok) {
        setErrors((current) => ({ ...current, [field]: undefined }));
        onChange({ [field]: result.chord });
      } else {
        setErrors((current) => ({ ...current, [field]: rejectionMessage(result) }));
      }
    } catch {
      setErrors((current) => ({ ...current, [field]: "无法检查快捷键，请稍后重试" }));
    }
  };
  const title = voiceOnly ? "自定义语音快捷键" : "切换热键";
  const rows = voiceOnly
    ? [{ field: "toggle_voice_input" as const, title: "语音输入开始／结束" }]
    : ROWS;
  return (
    <GroupList title={title}>
      <SettingsRowStack role="group" aria-label={title}>
        <SettingsGroupNote>
          点击后按下新的组合键完成重绑，Esc 取消，Backspace / Delete
          解绑；写法与冲突判定与保存时相同。
        </SettingsGroupNote>
        {rows.map(({ field, title }) => (
          <Row key={field} title={title}>
            <KeyChordRecorder
              value={keybindings[field] ?? ""}
              error={errors[field]}
              disabled={disabled}
              onCommit={(chord) => void commit(field, chord)}
            />
          </Row>
        ))}
      </SettingsRowStack>
    </GroupList>
  );
}
