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
  | "temporary_japanese"
  | "expression"
  | "command"
  | "mention"
  | "mention_places";

export type LocalModePreferences = {
  unicode: boolean;
  date_time: boolean;
  quick_phrase: boolean;
  emoji: boolean;
  kaomoji: boolean;
  super_jianpin: boolean;
  temporary_english: boolean;
  temporary_japanese: boolean;
  /** The V, / and @ modes. Off in a fresh profile and absent from the document while off, so an absent key reads as off. */
  expression?: boolean;
  command?: boolean;
  mention?: boolean;
  /** The @ mode also offers China's provinces, cities and counties from the built-in table, after the user's own names. Absent while off, like the three above. */
  mention_places?: boolean;
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

/** The modes a host has to route itself: digits that stay input in V, and the / and @ keys. Shown only where the host does (`HostCapabilities.plugin_triggers`). */
const triggerModeRows: readonly [LocalModeKey, string, string][] = [
  [
    "expression",
    "计算与数字(V 模式)",
    "中文模式下按 Shift+V，再输入算式（如 1+2*3）、数字（123 可转为一百二十三、壹佰贰拾叁或金额）或日期（2026.10.1）。空格上屏；Shift+数字选词；上屏内容不参与学习和打字统计",
  ],
  [
    "command",
    "指令(/ 模式)",
    "中文标点下没有输入时按 /，再输入指令字母：rq 日期、sj 时间、xq 星期、fy 翻译（fy 后输入英文，' 分隔单词，首选为中文译文），以及「插件」页启用的指令表。空格上屏；数字选词",
  ],
  [
    "mention",
    "@ 名字与地点(@ 模式)",
    "中文标点下没有输入时按 @，再输入拼音或首字母，从「插件」页的 @ 名单中选择。空格上屏；数字选词",
  ],
];

/** Shown under the @ switch wherever that is, and switchable only while it is on: the places come after the user's own names in the same mode. */
const mentionPlacesRow: [LocalModeKey, string, string] = [
  "mention_places",
  "@ 地名",
  "@ 模式在名单之后列出全国省、市、区县，候选旁注明所属省市。地名表内置在输入法中，不联网、不读取位置",
];

/** Why /fy gives nothing while no translation service is chosen: it asks the chosen service only, and never falls back to another. */
const commandTranslationNotice =
  "fy 翻译需要先在「标点与翻译 → 候选词翻译」选择翻译服务，目前未选择，fy 不会出结果。";

const iosLocalModeDescriptions: Partial<Record<LocalModeKey, string>> = {
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
  /** The host routes the V, / and @ modes, so their switches have an effect. */
  triggers?: boolean;
  /** The host can also edit the @ name list (a plugin store), without which the @ mode could never produce a candidate; its switch is shown only then. */
  mentions?: boolean;
  /** A translation service is chosen, which the / mode's fy command asks. When false, the / row says fy gives nothing; absent where the host offers no choice of service. */
  translationService?: boolean;
  onChange: (preferences: LocalModePreferences) => void;
}

/** Shared local input mode switches for desktop and touch settings hosts: the 实用功能 group. */
export function LocalModesSection({
  preferences,
  ios,
  triggers = false,
  mentions = false,
  translationService,
  onChange,
}: LocalModesSectionProps) {
  const rows = triggers
    ? [
        ...localModeRows,
        ...triggerModeRows.filter(([key]) => key !== "mention" || mentions),
        ...(mentions ? [mentionPlacesRow] : []),
      ]
    : localModeRows;
  return (
    <GroupList title="实用功能">
      {rows.map(([key, label, description]) => (
        <Row
          key={key}
          title={label}
          description={
            key === "command" && translationService === false
              ? `${description}。${commandTranslationNotice}`
              : ios
                ? (iosLocalModeDescriptions[key] ?? description)
                : description
          }
        >
          <Switch
            checked={preferences[key] ?? false}
            disabled={key === "mention_places" && !preferences.mention}
            onChange={(checked) => onChange({ ...preferences, [key]: checked })}
          />
        </Row>
      ))}
    </GroupList>
  );
}
