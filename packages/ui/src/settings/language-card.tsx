import { useState } from "react";
import type { Preferences } from "../core/preferences-model";
import { ActionSheet, type SheetOption } from "../core/action-sheet";
import { ChevronGlyph } from "../core/chevron-glyph";
import { PlusGlyph } from "../core/plus-glyph";
import {
  languageRemovalFallback,
  offeredLanguageSchemes,
  shownLanguageScheme,
  touchKeyboardLanguages,
  type TouchKeyboardLanguage,
  type TouchKeyboardLanguageId,
  type TouchKeyboardScheme,
} from "./touch-keyboard-scheme-helpers";

type WubiProfile = NonNullable<Preferences["wubi_profile"]>;

const shuangpinSchemes = ["xiaohe", "ziranma", "microsoft", "shoudao"] as const;
type ShuangpinScheme = (typeof shuangpinSchemes)[number];

const shuangpinNames: Record<ShuangpinScheme, string> = {
  xiaohe: "小鹤",
  ziranma: "自然码",
  microsoft: "微软",
  shoudao: "首道",
};

const wubiProfiles: readonly WubiProfile[] = ["wubi86", "wubi98"];

function isShuangpin(scheme: TouchKeyboardScheme): scheme is ShuangpinScheme {
  return (shuangpinSchemes as readonly string[]).includes(scheme);
}

function wubiName(profile: Preferences["wubi_profile"]): string {
  return profile === "wubi98" ? "五笔 98" : "五笔 86";
}

/** 方案在其所属语言内的名称：要简短，因为行或弹窗标题已经写出了语言。 */
function schemeName(scheme: TouchKeyboardScheme, wubiProfile: Preferences["wubi_profile"]): string {
  if (isShuangpin(scheme)) return shuangpinNames[scheme];
  switch (scheme) {
    case "quanpin":
      return "全拼";
    case "nine_key":
      return "全拼 9 键";
    case "nine_key_shuangpin":
      return "九键双拼";
    case "fourteen_key":
      return "全拼 14 键";
    case "eighteen_key":
      return "全拼 18 键";
    case "wubi":
      return wubiName(wubiProfile);
    case "zhuyin":
      return "注音";
    case "zhuyin_nine_key":
      return "注音 9 键";
    case "stroke":
      return "笔画";
    case "handwriting":
      return "手写";
    case "cantonese":
      return "粤拼";
    case "cantonese_shuangpin":
      return "粤语双拼";
    case "japanese_nine_key":
      return "9 键";
    case "japanese":
    case "korean":
    case "vietnamese":
    case "tibetan":
      return "26 键";
  }
}

/** 语言行为其方案显示的值；双拼方案连同其系列一起命名，如设计稿写的「双拼 · 小鹤」。 */
function rowValue(scheme: TouchKeyboardScheme, wubiProfile: Preferences["wubi_profile"]): string {
  return isShuangpin(scheme) ? `双拼 · ${shuangpinNames[scheme]}` : schemeName(scheme, wubiProfile);
}

const REMOVE = "remove";
const WUBI_PREFIX = "wubi:";

const row =
  "m-0 flex min-h-[52px] w-full cursor-pointer items-center gap-3 border-0 bg-transparent px-4 py-0 text-left [font-family:inherit] [color:var(--p-text)] active:bg-[var(--p-press)]";
/** 第一行之后每个语言行上方的细线，从名称开始的位置起画（16px 内边距、30px 徽标、12px 间距）。 */
const languageRule =
  "[background-image:linear-gradient(var(--p-hair),var(--p-hair))] [background-position:right_top] [background-size:calc(100%_-_58px)_0.5px] [background-repeat:no-repeat]";
/** 方案行下方的行（可添加的方案和「添加方案」开关）使用通栏细线，与设计稿一致。 */
const actionRule = "[box-shadow:inset_0_1px_0_var(--p-hair)]";
const badge =
  "flex size-[30px] flex-none items-center justify-center rounded-lg bg-[var(--accent-soft)] text-[15px] font-semibold [color:var(--p-accent-text)]";
const outlinedBadge =
  "flex size-[30px] flex-none items-center justify-center rounded-lg text-[15px] font-semibold [box-shadow:inset_0_0_0_1.5px_var(--p-hair)] [color:var(--p-sub)]";
const name = "min-w-0 flex-1 truncate [font-size:var(--p-row-fs)]";
const value = "shrink-0 whitespace-nowrap text-[15px] [color:var(--p-sub)]";

export interface LanguageCardProps {
  /** 本宿主和版本提供的触控方案，已按方案列表的方式过滤（`InputSchemeSettingsContent`）。 */
  available: readonly TouchKeyboardScheme[];
  enabled: readonly TouchKeyboardScheme[];
  selected: TouchKeyboardScheme;
  wubiProfile: Preferences["wubi_profile"];
  /** 把某个方案设为键盘的当前方案，若它处于关闭状态则先启用。 */
  onSelect: (scheme: TouchKeyboardScheme) => void;
  onToggle: (scheme: TouchKeyboardScheme, enabled: boolean) => void;
  onWubiProfileChange: (profile: WubiProfile) => void;
}

/**
 * 三端手机共用的「语言与方案」卡片。中文的全拼、双拼、五笔、注音、笔画和手写各自成行，粤拼也直接作为方案显示；日语、韩语、越南语和藏语仍按语言成行。点开一行可选该行的方案（双拼直接列小鹤、自然码等，五笔直接列 86 和 98，都不再套子菜单），移除会把这一行的所有方案从键盘上撤下。「添加方案」列出没有任何方案开启的行，添加时开启其第一个方案但不切换过去。
 *
 * 在弹窗里选中的方案若处于关闭状态会先启用。卡片绝不会让键盘没有方案：当某一行持有仅有的已启用方案时，不能移除它。
 */
export function LanguageCard({
  available,
  enabled,
  selected,
  wubiProfile,
  onSelect,
  onToggle,
  onWubiProfileChange,
}: LanguageCardProps) {
  const [adding, setAdding] = useState(false);
  const [sheet, setSheet] = useState<TouchKeyboardLanguageId | null>(null);
  const offered = touchKeyboardLanguages
    .map((language) => ({
      language,
      schemes: offeredLanguageSchemes(language, available),
      shown: shownLanguageScheme(language, selected, enabled, available),
    }))
    .filter(({ schemes }) => schemes.length > 0);
  // 有任一方案开启的行直接显示，其余行列在「添加方案」下。
  const listed = offered.filter(({ shown }) => shown);
  const addable = offered.filter(({ shown }) => !shown);
  const open = offered.find(({ language }) => language.id === sheet);

  const selectedIn = (scheme: TouchKeyboardScheme) =>
    selected === scheme && enabled.includes(scheme);

  const sheetOptions = (
    language: TouchKeyboardLanguage,
    schemes: readonly TouchKeyboardScheme[],
  ) => {
    const options: SheetOption[] = [];
    const plain = (scheme: TouchKeyboardScheme): SheetOption => ({
      value: scheme,
      label: schemeName(scheme, wubiProfile),
      selected: selectedIn(scheme),
    });
    // 每行的方案直接列在这一层：双拼列小鹤、自然码等，五笔列 86 和 98。再套一层子菜单，「双拼」行、方案面板、双拼面板就成了三层。
    if (language.id === "mandarin_wubi" && schemes.includes("wubi")) {
      // 选中 86 或 98 即保存版本并切到五笔。
      const wubi = selectedIn("wubi");
      const profile = wubiProfile ?? "wubi86";
      options.push(
        ...wubiProfiles.map((option) => ({
          value: `${WUBI_PREFIX}${option}`,
          label: wubiName(option),
          selected: wubi && option === profile,
        })),
      );
    } else {
      options.push(...schemes.map(plain));
    }
    options.push({
      value: REMOVE,
      label: `移除${language.title}`,
      destructive: true,
      disabled: languageRemovalFallback(language, enabled, available) === null,
    });
    return options;
  };

  const choose = (language: TouchKeyboardLanguage, choice: string) => {
    if (choice === REMOVE) {
      const fallback = languageRemovalFallback(language, enabled, available);
      if (!fallback) return;
      // 先切走，这样下面每次移除时键盘都停在一个会保留的方案上，也不会改动选择。
      if (language.schemes.includes(selected)) onSelect(fallback);
      for (const scheme of language.schemes) {
        if (enabled.includes(scheme)) onToggle(scheme, false);
      }
      return;
    }
    if (choice.startsWith(WUBI_PREFIX)) {
      onWubiProfileChange(choice.slice(WUBI_PREFIX.length) as WubiProfile);
      onSelect("wubi");
      return;
    }
    onSelect(choice as TouchKeyboardScheme);
  };

  return (
    // 与它替代的方案列表一样命名为「输入方案」：它就是同一个控件，只是按语言来画。
    <div role="group" aria-label="输入方案" className="flex min-w-0 flex-col">
      {listed.map(({ language, shown }, index) => (
        <button
          key={language.id}
          type="button"
          className={`${row} ${index > 0 ? languageRule : ""}`}
          aria-haspopup="dialog"
          aria-expanded={sheet === language.id}
          onClick={() => setSheet(language.id)}
        >
          <span className={badge} aria-hidden="true">
            {language.badge}
          </span>
          <span className={name}>{language.title}</span>
          {shown && <span className={value}>{rowValue(shown, wubiProfile)}</span>}
          <ChevronGlyph size={14} className="flex-none [color:var(--p-sub)]" />
        </button>
      ))}
      {adding &&
        addable.map(({ language, schemes }) => (
          <button
            key={language.id}
            type="button"
            className={`${row} ${actionRule}`}
            aria-label={`添加${language.title}`}
            onClick={() => {
              onToggle(schemes[0], true);
              // 添加最后一个可添加的方案后就无事可做，开关也随之消失；之后再移除时会重新从「添加方案」开始。
              if (addable.length === 1) setAdding(false);
            }}
          >
            <span className={outlinedBadge} aria-hidden="true">
              {language.badge}
            </span>
            <span className={`${name} [color:var(--p-sub)]`}>{language.title}</span>
            <span className="shrink-0 text-[15px] [color:var(--p-accent-text)]" aria-hidden="true">
              添加
            </span>
          </button>
        ))}
      {addable.length > 0 && (
        <button
          type="button"
          className={`${row} ${actionRule} [color:var(--p-accent-text)]`}
          aria-expanded={adding}
          onClick={() => setAdding((current) => !current)}
        >
          <span
            className="flex size-[30px] flex-none items-center justify-center"
            aria-hidden="true"
          >
            <PlusGlyph variant="row" size={20} strokeWidth={2.2} />
          </span>
          <span className="[font-size:var(--p-row-fs)]">{adding ? "完成" : "添加方案"}</span>
        </button>
      )}
      <ActionSheet
        open={open !== undefined}
        title={open?.language.title ?? ""}
        subtitle={open && open.schemes.length > 1 ? "选择输入方案" : undefined}
        options={open ? sheetOptions(open.language, open.schemes) : []}
        onSelect={(choice) => {
          if (open) choose(open.language, choice);
        }}
        onClose={() => setSheet(null)}
      />
    </div>
  );
}
