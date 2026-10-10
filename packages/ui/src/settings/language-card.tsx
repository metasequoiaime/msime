import { useState } from "react";
import type { Preferences } from "../index";
import { ActionSheet, type SheetOption } from "../core/action-sheet";
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
    case "fourteen_key":
      return "全拼 14 键";
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
/** 语言下方的行（可添加的语言和「添加语言」开关）使用通栏细线，与设计稿一致。 */
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
 * HarmonyOS 手机的「语言与方案」卡片，是 Android `TypingPage` 语言列表的 Web 版本。触控方案按语言分组；每行显示该语言的当前方案，点开弹窗可选另一个（双拼和五笔为子菜单），普通话以外的语言可在弹窗里移除，这会把它的所有方案从键盘上撤下。「添加语言」列出没有任何方案开启的语言，添加时开启其第一个方案但不切换过去。
 *
 * 在弹窗里选中的方案若处于关闭状态会先启用，所以普通话的弹窗列出宿主提供的所有方案。卡片绝不会让键盘没有方案：当某语言持有仅有的已启用方案时，不能移除它。
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
  // 普通话始终有一行；其他语言在有任一方案开启时有一行，否则列在「添加语言」下。
  const listed = offered.filter(({ language, shown }) => language.id === "mandarin" || shown);
  const addable = offered.filter(({ language, shown }) => language.id !== "mandarin" && !shown);
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
    if (language.id !== "mandarin") {
      options.push(...schemes.map(plain));
      options.push({
        value: REMOVE,
        label: `移除${language.title}`,
        destructive: true,
        disabled: languageRemovalFallback(language, enabled, available) === null,
      });
      return options;
    }
    for (const scheme of schemes) {
      if (scheme === "quanpin" || scheme === "fourteen_key" || scheme === "nine_key")
        options.push(plain(scheme));
    }
    const shuangpin = schemes.filter(isShuangpin);
    if (shuangpin.length > 0) {
      const current =
        shuangpin.find(selectedIn) ??
        shuangpin.find((scheme) => enabled.includes(scheme)) ??
        shuangpin[0];
      options.push({
        value: "shuangpin",
        label: `双拼（${shuangpinNames[current]}）`,
        selected: shuangpin.some(selectedIn),
        submenu: {
          title: "双拼",
          options: shuangpin.map((scheme) => ({
            value: scheme,
            label: shuangpinNames[scheme],
            selected: selectedIn(scheme),
          })),
        },
      });
    }
    if (schemes.includes("wubi")) {
      const wubi = selectedIn("wubi");
      const profile = wubiProfile ?? "wubi86";
      options.push({
        value: "wubi",
        label: `五笔（${wubiName(profile)}）`,
        selected: wubi,
        submenu: {
          title: "五笔",
          options: wubiProfiles.map((option) => ({
            value: `${WUBI_PREFIX}${option}`,
            label: wubiName(option),
            selected: wubi && option === profile,
          })),
        },
      });
    }
    for (const scheme of schemes) {
      if (
        scheme === "zhuyin" ||
        scheme === "zhuyin_nine_key" ||
        scheme === "stroke" ||
        scheme === "handwriting"
      )
        options.push(plain(scheme));
    }
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
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.4"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
            className="flex-none [color:var(--p-sub)]"
          >
            <path d="m9 6 6 6-6 6" />
          </svg>
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
              // 添加最后一个可添加的语言后就无事可做，开关也随之消失；之后再移除时会重新从「添加语言」开始。
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
            <svg
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.2"
              strokeLinecap="round"
            >
              <path d="M12 5v14M5 12h14" />
            </svg>
          </span>
          <span className="[font-size:var(--p-row-fs)]">{adding ? "完成" : "添加语言"}</span>
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
