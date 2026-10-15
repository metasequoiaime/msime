import { GroupList } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";
import { localModeRowsFor } from "./local-mode-options";
import { localModeControlState, type LocalModePreferences } from "./local-mode-preferences";
export {
  defaultLocalModes,
  type LocalModeKey,
  type LocalModePreferences,
} from "./local-mode-preferences";

export interface LocalModesSectionProps {
  preferences: LocalModePreferences;
  ios: boolean;
  /** The host routes the V, / and @ modes, so their switches have an effect. */
  triggers?: boolean;
  /** The host can also edit the @ name list (a plugin store), without which the @ mode could never produce a candidate; its switch is shown only then. */
  mentions?: boolean;
  /** A translation service is chosen, which the / mode's fy command asks. When false, the / row says fy gives nothing; absent where the host offers no choice of service. */
  translationService?: boolean;
  /** 本版本带临时日文（`HostCapabilities.edition.temporary_japanese`）；为 false 时不列出临时日语开关。缺省为 true。 */
  temporaryJapanese?: boolean;
  /** 显示「Shift+字母进入快捷模式」总开关（`HostCapabilities.shift_letter_entry_switch`，三个桌面宿主）。触屏键盘从菜单打开这些模式，不显示它。它同时区分桌面和触屏：拆字反查缺键时桌面读作关、触屏读作开，和 host-api 按宿主形态取的缺省一致。 */
  shiftLetterEntrySwitch?: boolean;
  onChange: (preferences: LocalModePreferences) => void;
}

/** 桌面和触屏宿主共用的本地输入模式开关：输入页「快捷模式」组。组名不带「Shift+字母」：iOS 从「更多 → 本地输入」进入，V、/、@ 三种也不是 Shift 组合。 */
export function LocalModesSection({
  preferences,
  ios,
  triggers = false,
  mentions = false,
  translationService,
  temporaryJapanese = true,
  shiftLetterEntrySwitch = false,
  onChange,
}: LocalModesSectionProps) {
  const rows = localModeRowsFor({
    temporaryJapanese,
    triggers,
    mentions,
    ios,
    translationService,
  });
  return (
    <GroupList title="快捷模式">
      {shiftLetterEntrySwitch && (
        <SwitchRow
          title="Shift+字母进入快捷模式"
          description="没有输入时按 Shift 加下面各模式的字母进入该模式。关掉后 Shift+字母照常打出大写字母，下面各模式的开关保留，/ 和 @ 不受影响"
          checked={preferences.shift_letter_entry ?? true}
          onChange={(checked) => onChange({ ...preferences, shift_letter_entry: checked })}
        />
      )}
      {rows.map(([key, label, description]) => (
        <SwitchRow
          key={key}
          title={label}
          description={description}
          {...localModeControlState(preferences, key, shiftLetterEntrySwitch)}
          onChange={(checked) => onChange({ ...preferences, [key]: checked })}
        />
      ))}
    </GroupList>
  );
}
