import { GroupList } from "../core/platform-controls";
import { iosLocalModeEntry } from "./local-mode-text";
import { SwitchRow } from "./switch-row";

export type LocalModeKey =
  | "unicode"
  | "date_time"
  | "quick_phrase"
  | "emoji"
  | "kaomoji"
  | "super_jianpin"
  | "temporary_english"
  | "temporary_japanese"
  | "chaizi"
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
  /** 「Shift+字母进入快捷模式」总开关。和上面几项相反，缺省读作开：早于它的文档里没有这个键。 */
  shift_letter_entry?: boolean;
  /** 拆字反查（C 模式）。缺键表示用户没动过开关，由宿主形态决定：触屏宿主读作开，桌面宿主读作关。只有用户动过开关时才写进文档。 */
  chaizi?: boolean;
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
    "中文模式下按 Shift+T，再输入 rq / riqi / date 输入日期，sj / shijian / time 输入时间，xq / xingqi / week 输入星期，nl / nongli / yinli 输入农历。打开时，拼音（含九键）直接输入 riqi / rq、shijian / sj、xingqi / xq、nongli / nl，日期、时间、星期、农历也会排在对应的词后面",
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
  [
    "chaizi",
    "拆字反查(C 模式)",
    "中文模式下按 Shift+C，再输入字的各个部件的拼音，例如 huohuohuo 得 焱、nvzi 得 好。部件之间可以用 ' 分隔，ü 用 v。空格上屏；数字选词",
  ],
];

/** 从 Shift+字母进入的模式：总开关关掉时它们的开关保留原值，只是暂时不起作用。`/` 和 `@` 不在其中。 */
const shiftLetterModes: ReadonlySet<LocalModeKey> = new Set<LocalModeKey>([
  "unicode",
  "date_time",
  "quick_phrase",
  "emoji",
  "kaomoji",
  "super_jianpin",
  "temporary_english",
  "temporary_japanese",
  "chaizi",
  "expression",
]);

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
    "中文标点下没有输入时按 /，再输入指令字母：rq 日期、sj 时间、xq 星期、nl 农历、fy 翻译（fy 后输入英文，' 分隔单词，首选为中文译文），以及「插件」页启用的指令表。空格上屏；数字选词",
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
  date_time: `${iosLocalModeEntry("日期时间")}再输入 rq / riqi / date 输入日期，sj / shijian / time 输入时间，xq / xingqi / week 输入星期，nl / nongli / yinli 输入农历。打开时，拼音（含九键）直接输入 riqi / rq、shijian / sj、xingqi / xq、nongli / nl，日期、时间、星期、农历也会排在对应的词后面`,
  unicode: `${iosLocalModeEntry("Unicode 码点")}再输入十六进制码位（如 4e00 / +1f600）。空格或点候选上屏`,
  emoji: `${iosLocalModeEntry("表情")}再输入全拼 / 简拼 / 双拼 / 英文关键词。空格或点候选上屏`,
  kaomoji: `${iosLocalModeEntry("颜文字")}再输入全拼 / 简拼 / 双拼 / 英文关键词。空格或点候选上屏`,
  super_jianpin: `${iosLocalModeEntry("超级简拼")}每个字母作为简拼；双拼按当前方案转换声母。空格或点候选上屏`,
  temporary_english: `${iosLocalModeEntry("英文补全")}之后按英文处理。空格上屏当前输入，也可以点候选；上屏后回到中文`,
  temporary_japanese: `${iosLocalModeEntry("临时日语")}之后按日语罗马字处理。空格上屏首选，也可以点候选；上屏后回到中文`,
  chaizi: `${iosLocalModeEntry("拆字")}再输入字的各个部件的拼音，例如 huohuohuo 得 焱、nvzi 得 好。部件之间可以用 ' 分隔，ü 用 v。空格或点候选上屏`,
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
  // 只有显示总开关的宿主才按它禁用各行：别的宿主上它永远是缺省的开。
  const shiftLetterEntry = !shiftLetterEntrySwitch || (preferences.shift_letter_entry ?? true);
  const modeRows = temporaryJapanese
    ? localModeRows
    : localModeRows.filter(([key]) => key !== "temporary_japanese");
  const rows = triggers
    ? [
        ...modeRows,
        ...triggerModeRows.filter(([key]) => key !== "mention" || mentions),
        ...(mentions ? [mentionPlacesRow] : []),
      ]
    : modeRows;
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
          description={
            key === "command" && translationService === false
              ? `${description}。${commandTranslationNotice}`
              : ios
                ? (iosLocalModeDescriptions[key] ?? description)
                : description
          }
          checked={preferences[key] ?? (key === "chaizi" && !shiftLetterEntrySwitch)}
          disabled={
            (key === "mention_places" && !preferences.mention) ||
            (!shiftLetterEntry && shiftLetterModes.has(key))
          }
          onChange={(checked) => onChange({ ...preferences, [key]: checked })}
        />
      ))}
    </GroupList>
  );
}
