import { GroupList, Row, Switch } from "../core/platform-controls";
import { iosLocalModeEntry } from "./local-mode-text";

export type LocalModeKey =
  | "unicode"
  | "date_time"
  | "quick_phrase"
  | "emoji"
  | "kaomoji"
  | "super_jianpin"
  | "temporary_english"
  | "temporary_japanese";

export type LocalModePreferences = {
  unicode: boolean;
  date_time: boolean;
  quick_phrase: boolean;
  emoji: boolean;
  kaomoji: boolean;
  super_jianpin: boolean;
  temporary_english: boolean;
  temporary_japanese: boolean;
};

export const defaultLocalModes: LocalModePreferences = {
  unicode: true,
  date_time: true,
  quick_phrase: true,
  emoji: true,
  kaomoji: true,
  super_jianpin: true,
  temporary_english: true,
  temporary_japanese: true,
};

const localModeRows: readonly [LocalModeKey, string, string][] = [
  ["quick_phrase", "快捷短语(K 模式)", "中文模式下按 Shift+K，再输入编码即可调用快捷短语"],
  [
    "date_time",
    "日期与时间快捷输入(T 模式)",
    "中文模式下按 Shift+T，再输入 rq / riqi / date 输入日期，sj / shijian / time 输入时间，xq / xingqi / week 输入星期",
  ],
  [
    "unicode",
    "Unicode 便捷录入(U 模式)",
    "中文模式下按 Shift+U，再输入十六进制码位（如 4e00 / +1f600）。空格上屏；Shift+数字选词",
  ],
  [
    "emoji",
    "Emoji 快捷输入(E 模式)",
    "中文模式下按 Shift+E，再输入全拼 / 简拼 / 双拼 / 英文关键词。空格上屏；数字选词",
  ],
  [
    "kaomoji",
    "颜文字快捷输入(M 模式)",
    "中文模式下按 Shift+M，再输入全拼 / 简拼 / 双拼 / 英文关键词。空格上屏；数字选词",
  ],
  [
    "super_jianpin",
    "超级简拼(J 模式)",
    "中文模式下按 Shift+J，每个字母作为简拼；双拼按当前方案转换声母。空格上屏；数字选词",
  ],
  [
    "temporary_english",
    "临时英文(Y 模式)",
    "中文模式下按 Shift+Y，之后按英文处理。空格上屏当前输入；数字选词；上屏后回到中文",
  ],
  [
    "temporary_japanese",
    "临时日语(R 模式)",
    "中文模式下按 Shift+R，之后按日语罗马字处理。空格上屏首选；数字选词；上屏后回到中文",
  ],
];

const iosLocalModeDescriptions: Record<LocalModeKey, string> = {
  quick_phrase: `${iosLocalModeEntry("快捷短语")}再输入编码即可调用快捷短语`,
  date_time: `${iosLocalModeEntry("日期时间")}再输入 rq / riqi / date 输入日期，sj / shijian / time 输入时间，xq / xingqi / week 输入星期`,
  unicode: `${iosLocalModeEntry("Unicode 码点")}再输入十六进制码位（如 4e00 / +1f600）。空格或点候选上屏`,
  emoji: `${iosLocalModeEntry("表情")}再输入全拼 / 简拼 / 双拼 / 英文关键词。空格或点候选上屏`,
  kaomoji: `${iosLocalModeEntry("颜文字")}再输入全拼 / 简拼 / 双拼 / 英文关键词。空格或点候选上屏`,
  super_jianpin: `${iosLocalModeEntry("超级简拼")}每个字母作为简拼；双拼按当前方案转换声母。空格或点候选上屏`,
  temporary_english: `${iosLocalModeEntry("英文补全")}之后按英文处理。空格上屏当前输入，也可以点候选；上屏后回到中文`,
  temporary_japanese: `${iosLocalModeEntry("临时日语")}之后按日语罗马字处理。空格上屏首选，也可以点候选；上屏后回到中文`,
};

export interface LocalModesSectionProps {
  preferences: LocalModePreferences;
  ios: boolean;
  onChange: (preferences: LocalModePreferences) => void;
}

/** Shared local input mode switches for desktop and touch settings hosts: the 实用功能 group. */
export function LocalModesSection({ preferences, ios, onChange }: LocalModesSectionProps) {
  return (
    <GroupList title="实用功能">
      {localModeRows.map(([key, label, description]) => (
        <Row
          key={key}
          title={label}
          description={ios ? iosLocalModeDescriptions[key] : description}
        >
          <Switch
            checked={preferences[key]}
            onChange={(checked) => onChange({ ...preferences, [key]: checked })}
          />
        </Row>
      ))}
    </GroupList>
  );
}
