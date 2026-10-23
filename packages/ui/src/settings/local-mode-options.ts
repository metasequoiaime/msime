import { iosLocalModeEntry } from "./local-mode-text";
import type { LocalModeKey } from "./local-mode-preferences";

export type LocalModeRow = readonly [LocalModeKey, string, string];

const localModeRows: readonly LocalModeRow[] = [
  ["quick_phrase", "快捷短语(K 模式)", "中文模式下按 Shift+K，再输入编码即可调用快捷短语"],
  [
    "date_time",
    "日期与时间快捷输入(T 模式)",
    "中文模式下按 Shift+T，再输入 rq / riqi / date 输入日期，sj / shijian / time 输入时间，xq / xingqi / week 输入星期，nl / nongli / yinli 输入农历",
  ],
  [
    "date_time_inline",
    "日期时间混入候选",
    "全拼直接输入 riqi / rq、shijian / sj、xingqi / xq、nongli / nl 时，当前日期、时间、星期、农历接在对应的词后面；与 T 模式是两个开关",
  ],
  [
    "date_time_menu",
    "日期格式入口",
    "日期时间混入候选时，在第三位放一个「📅日期」入口，选中后候选窗换成该组的全部日期格式，和 T 模式相同",
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
    "symbol",
    "符号快捷输入(S 模式)",
    "中文模式下按 Shift+S，再输入分组码（xl 希腊字母、sx 数学、szq 带圈数字、jt 箭头等）或符号名（alpha、infinity）。空格上屏；数字选词",
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

/** V、/ 和 @ 由宿主路由，只有宿主声明触发能力时显示。 */
const triggerModeRows: readonly LocalModeRow[] = [
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

/** 地名紧跟 @ 名字行，勾选和禁用状态由偏好模块统一计算。 */
const mentionPlacesRow: LocalModeRow = [
  "mention_places",
  "@ 地名",
  "@ 模式在名单之后列出全国省、市、区县，候选旁注明所属省市。地名表内置在输入法中，不联网、不读取位置",
];

const commandTranslationNotice =
  "fy 翻译需要先在「标点与翻译 → 候选词翻译」选择翻译服务，目前未选择，fy 不会出结果。";

const iosLocalModeDescriptions: Partial<Record<LocalModeKey, string>> = {
  quick_phrase: `${iosLocalModeEntry("快捷短语")}再输入编码即可调用快捷短语`,
  date_time: `${iosLocalModeEntry("日期时间")}再输入 rq / riqi / date 输入日期，sj / shijian / time 输入时间，xq / xingqi / week 输入星期，nl / nongli / yinli 输入农历`,
  unicode: `${iosLocalModeEntry("Unicode 码点")}再输入十六进制码位（如 4e00 / +1f600）。空格或点候选上屏`,
  emoji: `${iosLocalModeEntry("表情")}再输入全拼 / 简拼 / 双拼 / 英文关键词。空格或点候选上屏`,
  kaomoji: `${iosLocalModeEntry("颜文字")}再输入全拼 / 简拼 / 双拼 / 英文关键词。空格或点候选上屏`,
  symbol: `${iosLocalModeEntry("符号")}再输入分组码（xl 希腊字母、sx 数学、szq 带圈数字、jt 箭头等）或符号名（alpha、infinity）。空格或点候选上屏`,
  super_jianpin: `${iosLocalModeEntry("超级简拼")}每个字母作为简拼；双拼按当前方案转换声母。空格或点候选上屏`,
  temporary_english: `${iosLocalModeEntry("英文补全")}之后按英文处理。空格上屏当前输入，也可以点候选；上屏后回到中文`,
  temporary_japanese: `${iosLocalModeEntry("临时日语")}之后按日语罗马字处理。空格上屏首选，也可以点候选；上屏后回到中文`,
  chaizi: `${iosLocalModeEntry("拆字")}再输入字的各个部件的拼音，例如 huohuohuo 得 焱、nvzi 得 好。部件之间可以用 ' 分隔，ü 用 v。空格或点候选上屏`,
};

export function localModeRowsFor({
  temporaryJapanese,
  triggers,
  mentions,
  ios,
  quickPhraseCandidates = false,
  translationService,
}: {
  temporaryJapanese: boolean;
  triggers: boolean;
  mentions: boolean;
  ios: boolean;
  quickPhraseCandidates?: boolean;
  translationService?: boolean;
}): readonly LocalModeRow[] {
  const modes = temporaryJapanese
    ? localModeRows
    : localModeRows.filter(([key]) => key !== "temporary_japanese");
  const rows = triggers
    ? [
        ...modes,
        ...triggerModeRows.filter(([key]) => key !== "mention" || mentions),
        ...(mentions ? [mentionPlacesRow] : []),
      ]
    : modes;
  return rows.map(([key, label, description]) => [
    key,
    label,
    key === "quick_phrase" && quickPhraseCandidates
      ? "中文模式下直接输入编码即可调用快捷短语，也可以按 Shift+K 后输入编码"
      : key === "command" && translationService === false
        ? `${description}。${commandTranslationNotice}`
        : ios
          ? (iosLocalModeDescriptions[key] ?? description)
          : description,
  ]);
}
