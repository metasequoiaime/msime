import { useEffect, useId, useState, type CSSProperties } from "react";
import type { Preferences } from "../core/preferences-model";
import type { InputScheme } from "../core/input-scheme";
import {
  harmonyTouchKeyboardRows,
  harmonyTouchLayout,
  type HarmonyTouchLayout,
  type PreviewKey,
} from "../keyboard/keyboard-layouts";
import { GroupList, LinkRow, Segmented } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";
import { SettingsSearchReveal } from "./settings-search-reveal";
import { phoneHeatmapLayoutRows } from "./keyboard-heatmap";
import {
  inferredTouchKeyboardScheme,
  type TouchKeyboardScheme,
} from "./touch-keyboard-scheme-helpers";
import { useMobileSettingsSubview } from "./use-mobile-settings-subview";
import { NineKeySidebarSettings } from "./nine-key-sidebar-settings";
import {
  fourteenKeyBottomRows,
  fourteenKeyStyle,
  fourteenKeyStyleHighlights,
  fourteenKeyStyleNotes,
  fourteenKeyStyleOptions,
  nineKeySidebarStoredValue,
  nineKeySidebarSummary,
  nineKeySidebarSymbols,
  nineKeySidebarTitle,
  nineKeySidebarVisibleRows,
  languageKeyLeft,
  languageKeyLeftDescription,
  languageKeyLeftTitle,
  nineKeyStyle,
  nineKeyStyleHighlights,
  nineKeyStyleNotes,
  nineKeyStyleOptions,
  numberPadSpace,
  numberPadSpaceNote,
  numberPadSpaceOptions,
  numberPadSpaceTitle,
  numberPadSidebarSymbols,
  numberPadSidebarTitle,
  numberRowDescription,
  numberRowKeys,
  symbolFirstDescription,
  symbolFirstTitle,
  twentySixKeyBottomRows,
  twentySixKeyStyle,
  twentySixKeyStyleHighlights,
  twentySixKeyStyleNotes,
  twentySixKeyStyleOptions,
  storedNineKeySidebar,
  withLanguageKeyLeft,
  withSymbolFirst,
  type NineKeySidebarKind,
} from "../keyboard/touch-key-layout-styles";

/** 「按键布局」子页上提供哪几种键盘类型：版本没有的键盘类型不进分段控件。 */
export type TouchKeyLayoutGroups = {
  twentySix: boolean;
  fourteen: boolean;
  nine: boolean;
};

/** 子页分段控件的一段：26 键、14 键与 18 键（共用一个样式）、9 键。 */
export type TouchKeyLayoutKind = keyof TouchKeyLayoutGroups;

const touchKeyLayoutKinds: readonly TouchKeyLayoutKind[] = ["twentySix", "fourteen", "nine"];

/** 分段控件上的名字，三端相同。 */
export const touchKeyLayoutKindTitles: Readonly<Record<TouchKeyLayoutKind, string>> = {
  twentySix: "26 键",
  fourteen: "14/18 键",
  nine: "9 键",
};

/**
 * 按版本提供的方案决定提供哪几种键盘类型：中文 26 键来自全拼、双拼、五笔和粤拼；14 键与 18 键来自全拼、双拼（14/18 键）和五笔（14 键）；九键来自全拼九键、九键双拼和笔画。只有日文、韩文等非中文方案的版本一种都没有。
 */
export function touchKeyLayoutGroups(schemes: readonly InputScheme[]): TouchKeyLayoutGroups {
  const has = (...wanted: InputScheme[]) => wanted.some((scheme) => schemes.includes(scheme));
  return {
    twentySix: has("quanpin", "shuangpin", "wubi", "cantonese"),
    fourteen: has("quanpin", "shuangpin", "wubi"),
    nine: has("quanpin", "shuangpin", "stroke"),
  };
}

/**
 * 用户当前正在用的中文键盘属于哪一类：全拼、双拼、五笔、粤拼 26 键是 26 键；全拼 14 / 18 键、双拼 14 / 18 键、五笔 14 键是 14/18 键；全拼九键、九键双拼和笔画是 9 键。别的方案（日文、韩文、手写等）判断不了，返回 null，子页停在第一段。
 */
export function touchKeyLayoutKind(
  scheme: TouchKeyboardScheme,
  shuangpinLayout: Preferences["touch_shuangpin_layout"],
  wubiLayout: Preferences["touch_wubi_layout"],
): TouchKeyLayoutKind | null {
  switch (scheme) {
    case "nine_key":
    case "nine_key_shuangpin":
    case "stroke":
      return "nine";
    case "fourteen_key":
    case "eighteen_key":
      return "fourteen";
    case "xiaohe":
    case "ziranma":
    case "microsoft":
    case "shoudao":
      return shuangpinLayout === "fourteen_key" || shuangpinLayout === "eighteen_key"
        ? "fourteen"
        : "twentySix";
    case "wubi":
      return wubiLayout === "fourteen_key" ? "fourteen" : "twentySix";
    case "quanpin":
    case "cantonese":
      return "twentySix";
    default:
      return null;
  }
}

/** 预览里的一个键：`tone` 决定键帽颜色（字母键、功能键、强调色的回车），`highlighted` 是这个样式与「标准」不同、要描边的键，`rowSpan` 为 2 的键从所在行往下多占一行（九键「右侧大回车」的回车）。没有 `label` 的是占位，只占宽度不画键帽。`stack` 是一格里自上而下等高排的几个键：9 键左列换成用户的符号表时，整列画成跨网格各行的一格，里面是表里的前几项（`nineKeySidebarStack`）。 */
export type KeyLayoutPreviewKey = PreviewKey & {
  tone: "letter" | "function" | "return";
  highlighted: boolean;
  rowSpan?: number;
};

/**
 * 9 键预览的左列换成用户的字母层表（#10223）：左列整列是跨网格各行的一格，里面自上而下等高排表里的前 `nineKeySidebarVisibleRows` 项（与鸿蒙键盘一屏的项数相同），其余行的左列是占位；份额和键 id `SoftPunctuation` 照旧。空表时不调用，左列仍是热力图那几格出厂标点。
 */
function nineKeySidebarStack(
  grid: readonly (readonly PreviewKey[])[],
  symbols: readonly string[],
): PreviewKey[][] {
  const visible = symbols.slice(0, nineKeySidebarVisibleRows);
  return grid.map((row, index) => {
    const [side, ...rest] = row;
    const cell: PreviewKey =
      index === 0
        ? {
            label: visible.join(" "),
            weight: side.weight,
            code: side.code,
            rowSpan: grid.length,
            stack: visible,
          }
        : { label: "", weight: side.weight };
    return [cell, ...rest];
  });
}

/** 预览的一行：`numberRow` 为真的是字母上方的数字行，画得比字母行矮一些。 */
export type KeyLayoutPreviewRow = { keys: KeyLayoutPreviewKey[]; numberRow?: boolean };

/** 14/18 键预览画哪一种字母格：用户当前在用 18 键（全拼或双拼）时画 18 键，双拼画带 ； 的那一种，其余画全拼 14 键。 */
function fourteenKeyFace(
  scheme: TouchKeyboardScheme,
  preferences: Preferences,
): HarmonyTouchLayout {
  const face = harmonyTouchLayout(
    scheme,
    preferences.touch_shuangpin_layout,
    preferences.touch_wubi_layout,
  );
  return face === "eighteen_key" ||
    face === "fourteen_key_shuangpin" ||
    face === "eighteen_key_shuangpin"
    ? face
    : "fourteen_key";
}

/**
 * 鸿蒙手机键盘 `layout` 按用户的按键布局偏好排出的整块键盘（数字行、字母行、底行），「按键布局」子页的整块预览和其他设置页的键盘预览都从这里取，不另编一套：26 键、14 键和 18 键的字母行取 `harmonyTouchKeyboardRows`（照 HarmonyOS 键盘的键位和份额），底行取所选样式的 `twentySixKeyBottomRows` / `fourteenKeyBottomRows`（26 键打开 `touch_twenty_six_symbol_first` 时经 `withSymbolFirst` 把 符 挪到 123 左边；`touch_language_key_side` 为 `left` 时三类键盘再经 `withLanguageKeyLeft` 把 中/英 挪到空格左边，与键盘同一条规则），打开 `touch_number_row` 时在字母上方加 `numberRowKeys`；9 键取统计热力图照 HarmonyOS 键盘画的 `phoneHeatmapLayoutRows`（左侧符号栏、3 × 3 九宫格和右列 ⌫ / 分词 / ！，底行与「右侧大回车」的外框随样式变，中/英 换边同样由它排），用户换了字母层的左侧符号表（`touch_nine_key_sidebar_symbols`，#10223）时左列按那张表画一屏（`nineKeySidebarStack`），9 键不画数字行；日语 9 键是假名格，不受这些偏好影响，原样画。
 *
 * `chineseTwentySix` 为假时 26 键不按 `touch_twenty_six_key_style` 排，画 26 键一族的标准底行：与 HarmonyOS `designRowStyledFace` 相同，样式只作用于中文 26 键，日语等 26 键用标准底行，但「符」与「123」互换和中/英 换边照样生效（`designRowEntries` 对所有 26 键一族的底行都套 `symbolFirst` 和 `languageBeforeSpace`）。鸿蒙没有隐藏中/英键的设置，底行一直有 中。
 */
function harmonyStyledRows(
  layout: HarmonyTouchLayout,
  preferences: Preferences | undefined,
  chineseTwentySix: boolean,
): PreviewKey[][] {
  if (layout === "japanese_nine_key") return harmonyTouchKeyboardRows.japanese_nine_key;
  if (layout === "nine_key") {
    const rows = phoneHeatmapLayoutRows("nine", {
      twentySixKeyStyle: "standard",
      nineKeyStyle: preferences ? nineKeyStyle(preferences) : "standard",
      numberRow: false,
      languageKeyLeft: preferences ? languageKeyLeft(preferences) : false,
    });
    const sidebar = preferences ? storedNineKeySidebar(preferences, "letter") : [];
    if (sidebar.length === 0) return rows;
    return [...nineKeySidebarStack(rows.slice(0, -1), sidebar), rows[rows.length - 1]];
  }
  const letters = harmonyTouchKeyboardRows[layout].slice(0, -1);
  const bottom = withLanguageKeyLeft(
    layout === "twenty_six_key"
      ? withSymbolFirst(
          twentySixKeyBottomRows[
            preferences && chineseTwentySix ? twentySixKeyStyle(preferences) : "standard"
          ],
          preferences?.touch_twenty_six_symbol_first === true,
        )
      : fourteenKeyBottomRows[preferences ? fourteenKeyStyle(preferences) : "standard"],
    preferences ? languageKeyLeft(preferences) : false,
  );
  const numberRow = preferences?.touch_number_row === true ? [numberRowKeys] : [];
  return [...numberRow, ...letters, bottom];
}

/**
 * 鸿蒙手机设置页里键盘预览（屏幕键盘页「尺寸」、皮肤页的皮肤卡片与编辑器、社区皮肤卡片与详情）画的键：用户当前的中文键盘（`harmonyTouchLayout`），按他的按键布局偏好排（`harmonyStyledRows`），与「按键布局」子页、实际键盘的底行和数字行一致。预览不做高亮，高亮只在「按键布局」子页。
 *
 * `scheme` 缺省按偏好推断；屏幕键盘页传入它正显示为选中的方案。没有偏好可读时（`preferences` 为 undefined）画默认的全拼 26 键、标准底行、无数字行。
 */
export function harmonyTouchRowsForPreferences(
  preferences: Preferences | undefined,
  scheme: TouchKeyboardScheme = preferences ? inferredTouchKeyboardScheme(preferences) : "quanpin",
): PreviewKey[][] {
  const shuangpinLayout = preferences?.touch_shuangpin_layout;
  const wubiLayout = preferences?.touch_wubi_layout;
  const kind = touchKeyLayoutKind(scheme, shuangpinLayout, wubiLayout);
  // 九键双拼和笔画也画九宫格外框（字母格用拼音九键的 ABC），与 Android 的皮肤预览、「按键布局」页 9 键分段同一条规则；`harmonyTouchLayout` 只认拼音九键，其余会落到 26 键。
  return harmonyStyledRows(
    kind === "nine" ? "nine_key" : harmonyTouchLayout(scheme, shuangpinLayout, wubiLayout),
    preferences,
    kind === "twentySix",
  );
}

/**
 * 「按键布局」整块预览要画的键：`kind` 那一类键盘按用户偏好排出的整块键盘（`harmonyStyledRows`，与其他设置页的键盘预览同一张表），14/18 键画用户在用的那种字母格（`fourteenKeyFace`）。
 *
 * 键帽颜色按位置定：九宫格两侧的列、字母区第三行两端（⇧ / 符 和 ⌫）、底行除空格和回车外的键是功能键，回车用强调色。要高亮的键按所选样式的高亮表（`twentySixKeyStyleHighlights` 等）用键 id 认；26 键互换打开且样式有 符 时另加 符 和 123；中/英 在空格左边且底行里有 中/英 时另加 中/英（被挤动的空格和标点宽度、样子都没变，不加）。
 */
export function keyLayoutPreviewRows(
  kind: TouchKeyLayoutKind,
  preferences: Preferences,
  scheme: TouchKeyboardScheme,
): KeyLayoutPreviewRow[] {
  const layout: HarmonyTouchLayout =
    kind === "nine"
      ? "nine_key"
      : kind === "fourteen"
        ? fourteenKeyFace(scheme, preferences)
        : "twenty_six_key";
  const styled = harmonyStyledRows(layout, preferences, true);
  const numberRow = kind !== "nine" && preferences.touch_number_row === true;
  const grid = styled.slice(numberRow ? 1 : 0, -1);
  const bottom = styled[styled.length - 1];
  let highlights: readonly string[];
  const languageLeft = languageKeyLeft(preferences);
  if (kind === "nine") {
    highlights = nineKeyStyleHighlights[nineKeyStyle(preferences)];
  } else if (kind === "fourteen") {
    highlights = fourteenKeyStyleHighlights[fourteenKeyStyle(preferences)];
  } else {
    const style = twentySixKeyStyle(preferences);
    // 互换打开且这个样式有 符 时，符 和 123 也描边，让人看出开关动的是哪两个键。
    highlights =
      preferences.touch_twenty_six_symbol_first === true &&
      bottom.some((key) => key.code === "SoftSymbol")
        ? [...twentySixKeyStyleHighlights[style], "SoftSymbol", "SoftLayer"]
        : twentySixKeyStyleHighlights[style];
  }
  // 中/英 挪到空格左边时也描边它，让人看出开关动的是哪个键。
  if (languageLeft && bottom.some((key) => key.code === "SoftLanguage"))
    highlights = [...highlights, "SoftLanguage"];
  const highlighted = (key: PreviewKey) => key.code !== undefined && highlights.includes(key.code);
  // 用户换了左列的表时左列是一整格栈，整列描边（右列的 ！ 也记 `SoftPunctuation`，所以按位置认、不按键 id 加进高亮表）。
  const place = (key: PreviewKey, tone: KeyLayoutPreviewKey["tone"]): KeyLayoutPreviewKey => ({
    label: key.label,
    weight: key.weight,
    ...(key.code === undefined ? {} : { code: key.code }),
    ...(key.rowSpan === undefined ? {} : { rowSpan: key.rowSpan }),
    ...(key.stack === undefined ? {} : { stack: key.stack }),
    tone: key.code === "Enter" ? "return" : tone,
    highlighted: key.stack !== undefined || highlighted(key),
  });
  const rows: KeyLayoutPreviewRow[] = [];
  if (numberRow) rows.push({ keys: styled[0].map((key) => place(key, "letter")), numberRow: true });
  grid.forEach((row, rowIndex) => {
    const edges = kind === "nine" || rowIndex === grid.length - 1;
    rows.push({
      keys: row.map((key, index) =>
        place(key, edges && (index === 0 || index === row.length - 1) ? "function" : "letter"),
      ),
    });
  });
  rows.push({
    keys: bottom.map((key) => place(key, key.code === "Space" ? "letter" : "function")),
  });
  return rows;
}

/** 预览的几何：字母行按 1 份高、数字行 0.7 份，行距和外框内边距 5px；外框按 `PREVIEW_ASPECT_WIDTH` 宽、每份 `PREVIEW_ROW_UNIT` 高的比例随页面宽度等比缩放，手机竖屏（预览约 326px 宽）上四行约 185px 高、加数字行约 218px。 */
const PREVIEW_GAP_PX = 5;
const NUMBER_ROW_UNITS = 0.7;
const PREVIEW_ASPECT_WIDTH = 340;
const PREVIEW_ROW_UNIT = 42;

const toneClass: Record<KeyLayoutPreviewKey["tone"], string> = {
  letter: "[background:var(--p-seg-on)] [color:var(--p-text)]",
  function:
    "[background:color-mix(in_srgb,var(--p-seg-on)_88%,var(--p-text))] [color:var(--p-text)]",
  return: "bg-accent-strong text-white",
};

/** 跨两行的键（右侧大回车）往下伸到下一行的占位上：高度是两行加一道行距；各行按份额定高，所以它不撑高所在行。 */
function keySpan(rowSpan: number | undefined): CSSProperties {
  const span = rowSpan ?? 1;
  if (span <= 1) return {};
  return {
    height: `calc(${span * 100}% + ${(span - 1) * PREVIEW_GAP_PX}px)`,
    alignSelf: "flex-start",
  };
}

/** 整块键盘预览：画成真实键盘的样子，各行按份额铺满整行，这个样式与「标准」不同的键用强调色描边。只是示意，不可点，读屏跳过它（样式胶囊本身有名字，下方的说明讲副作用）。 */
function KeyLayoutPreview({
  kind,
  rows,
}: {
  kind: TouchKeyLayoutKind;
  rows: KeyLayoutPreviewRow[];
}) {
  const units = rows.reduce((sum, row) => sum + (row.numberRow ? NUMBER_ROW_UNITS : 1), 0);
  const height = units * PREVIEW_ROW_UNIT + PREVIEW_GAP_PX * (rows.length + 1);
  return (
    <div
      className="flex w-full flex-col rounded-[14px] [background:var(--p-bg)]"
      style={{
        gap: PREVIEW_GAP_PX,
        padding: PREVIEW_GAP_PX,
        aspectRatio: `${PREVIEW_ASPECT_WIDTH} / ${height}`,
      }}
      aria-hidden="true"
      data-key-layout-preview={kind}
    >
      {rows.map((row, rowIndex) => (
        <div
          key={rowIndex}
          className="flex min-h-0"
          style={{ gap: PREVIEW_GAP_PX, flex: `${row.numberRow ? NUMBER_ROW_UNITS : 1} 1 0%` }}
          data-key-layout-row={row.numberRow ? "number" : rowIndex}
        >
          {row.keys.map((key, index) =>
            key.stack ? (
              <span
                key={index}
                className="flex min-w-0 flex-col"
                style={{ flex: `${key.weight} 1 0%`, gap: PREVIEW_GAP_PX, ...keySpan(key.rowSpan) }}
                data-key-stack=""
                data-row-span={key.rowSpan}
                data-weight={key.weight}
              >
                {key.stack.map((label, at) => (
                  <span
                    key={at}
                    className={`flex min-h-0 flex-1 items-center justify-center overflow-hidden rounded-[6px] leading-none whitespace-nowrap [box-shadow:0_1px_1px_rgba(0,0,0,0.12)] ${[...label].length === 1 ? "text-[15px]" : "text-[12px]"} ${toneClass[key.tone]} ${key.highlighted ? "outline-2 outline-offset-1 outline-accent" : ""}`}
                    data-key-code={key.code}
                    data-key-tone={key.tone}
                    data-highlighted={key.highlighted ? "" : undefined}
                  >
                    {label}
                  </span>
                ))}
              </span>
            ) : key.label ? (
              <span
                key={index}
                className={`flex min-w-0 items-center justify-center overflow-hidden rounded-[6px] leading-none whitespace-nowrap [box-shadow:0_1px_1px_rgba(0,0,0,0.12)] ${[...key.label].length === 1 ? "text-[15px]" : "text-[12px]"} ${toneClass[key.tone]} ${key.highlighted ? "outline-2 outline-offset-1 outline-accent" : ""}`}
                style={{ flex: `${key.weight} 1 0%`, ...keySpan(key.rowSpan) }}
                data-key-code={key.code}
                data-key-tone={key.tone}
                data-highlighted={key.highlighted ? "" : undefined}
                data-row-span={key.rowSpan}
                data-weight={key.weight}
              >
                {key.label}
              </span>
            ) : (
              <span key={index} style={{ flex: `${key.weight} 1 0%` }} data-weight={key.weight} />
            ),
          )}
        </div>
      ))}
    </div>
  );
}

/** 一排等宽胶囊：等宽铺满一行、不横向滚动（每类最多四种、名字最长五个字，等分后放得下，字号很大时显示省略号）；选中的强调色实底白字，其余浅底。点一下立刻写偏好，点已选中的不做事。`label` 是读屏念的这排胶囊的名字，`note` 是胶囊下方的说明（样式胶囊是当前选中样式的副作用，数字键盘一组是组末说明），没有就不画这一行，读屏把它当作这排胶囊的描述。 */
function StyleChips<T extends string>({
  label,
  options,
  value,
  note,
  onChange,
}: {
  label: string;
  options: readonly { value: T; label: string }[];
  value: T;
  note: string | undefined;
  onChange: (value: T) => void;
}) {
  const noteId = useId();
  return (
    <>
      <div
        role="group"
        aria-label={label}
        aria-describedby={note ? noteId : undefined}
        className="flex gap-1.5"
      >
        {options.map((option) => {
          const on = option.value === value;
          return (
            <button
              key={option.value}
              type="button"
              aria-pressed={on}
              className={`m-0 min-w-0 flex-1 cursor-pointer overflow-hidden rounded-full border-0 px-1.5 py-1.5 text-[14px] text-ellipsis whitespace-nowrap [font-family:inherit] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent disabled:cursor-default disabled:opacity-50 ${on ? "bg-accent-strong font-medium text-white" : "bg-[var(--p-seg-bg)] [color:var(--p-text)]"}`}
              data-settings-search-label={option.label}
              data-key-layout-style={option.value}
              onClick={() => {
                if (!on) onChange(option.value);
              }}
            >
              {option.label}
            </button>
          );
        })}
      </div>
      {note && (
        <p
          id={noteId}
          className="m-0 [font-size:var(--p-sub-fs)] [color:var(--p-sub)]"
          data-key-layout-note=""
        >
          {note}
        </p>
      )}
    </>
  );
}

/**
 * 「按键布局」子页的内容，三端一致：最上面是键盘类型分段控件（26 键 | 14/18 键 | 9 键，只列版本提供的类型，只有一种时不画），默认停在用户当前在用的中文键盘类型；下面是这一类键盘的整块预览、一排样式胶囊和当前样式的说明，26 键分段在同一张卡片里再列「26 键「符」与「123」互换」开关，打开时预览把 符 画在 123 左边并描边这两个键；每个分段的卡片里都有「中/英键放在空格左边」开关（26 键分段里排在互换开关之后，三个分段是同一个偏好，切分段时状态不变），打开时预览把 中/英 画到空格左边并描边它；样式卡片下面是「数字键盘」一组（空格在右列、0 右边或 0 左边的一排胶囊加组末说明，没选过时选中鸿蒙原来的「空格在 0 左边」，不画预览）；最后单独一组是「顶部数字行」开关。切分段只换预览和胶囊，不写偏好；点胶囊写共享偏好 `touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style` 或 `touch_number_pad_space`，开关写 `touch_twenty_six_symbol_first`、`touch_language_key_side`（打开写 `left`、关上写 `right`）或 `touch_number_row`，键盘开着时当场换排法。
 *
 * 三类键盘的内容都挂着，只是不在当前分段的隐藏起来：设置项搜索因此扫得到每个样式名和互换开关，命中别的分段里的项时由 SettingsSearchReveal 切过去；「中/英键放在空格左边」每个分段都显示，命中它时停在当前分段（子页刚打开时是用户在用的那一类，判断不了时是第一段）。`open` 是子页是否打开，关上时分段回到默认，下次打开仍停在当前在用的键盘类型。
 */
export function TouchKeyLayoutSettings({
  preferences,
  scheme,
  groups,
  open,
  mobile = false,
  onChange,
}: {
  preferences: Preferences;
  /** 用户当前选中的触屏方案（`selectedTouchKeyboardScheme`），决定默认分段和 14/18 键画哪种字母格。 */
  scheme: TouchKeyboardScheme;
  groups: TouchKeyLayoutGroups;
  open: boolean;
  /** HarmonyOS 手机：两张左侧符号表的编辑页压一条历史记录，系统返回手势先关它。 */
  mobile?: boolean;
  onChange: (patch: Partial<Preferences>) => void;
}) {
  const offered = touchKeyLayoutKinds.filter((kind) => groups[kind]);
  const current = touchKeyLayoutKind(
    scheme,
    preferences.touch_shuangpin_layout,
    preferences.touch_wubi_layout,
  );
  const fallback = current && offered.includes(current) ? current : offered[0];
  const [chosen, setChosen] = useState<TouchKeyLayoutKind | null>(null);
  // 九键左侧符号的编辑页（#10223）是「按键布局」上面的又一层子页，和「按键布局」本身一样压一条历史记录。
  const {
    value: editing,
    open: openEditor,
    close: closeEditor,
    reset: resetEditor,
  } = useMobileSettingsSubview<NineKeySidebarKind>({
    mobile,
    stateKey: "nineKeySidebar",
    decode: (state) =>
      state?.nineKeySidebar === "letter" || state?.nineKeySidebar === "digit"
        ? state.nineKeySidebar
        : null,
    encode: (which) => ({ nineKeySidebar: which }),
  });
  useEffect(() => {
    if (open) return;
    setChosen(null);
    resetEditor();
  }, [open, resetEditor]);
  const saveSidebar = (which: NineKeySidebarKind, symbols: readonly string[]) => {
    const value = nineKeySidebarStoredValue(which, symbols);
    onChange(
      which === "letter"
        ? { touch_nine_key_sidebar_symbols: value }
        : { touch_nine_key_digit_sidebar_symbols: value },
    );
    closeEditor();
  };
  const kind = chosen && offered.includes(chosen) ? chosen : fallback;
  const chips = (panel: TouchKeyLayoutKind) => {
    const label = `${touchKeyLayoutKindTitles[panel]}样式`;
    if (panel === "twentySix") {
      const value = twentySixKeyStyle(preferences);
      return (
        <StyleChips
          label={label}
          options={twentySixKeyStyleOptions}
          value={value}
          note={twentySixKeyStyleNotes[value]}
          onChange={(touch_twenty_six_key_style) => onChange({ touch_twenty_six_key_style })}
        />
      );
    }
    if (panel === "fourteen") {
      const value = fourteenKeyStyle(preferences);
      return (
        <StyleChips
          label={label}
          options={fourteenKeyStyleOptions}
          value={value}
          note={fourteenKeyStyleNotes[value]}
          onChange={(touch_fourteen_key_style) => onChange({ touch_fourteen_key_style })}
        />
      );
    }
    const value = nineKeyStyle(preferences);
    return (
      <StyleChips
        label={label}
        options={nineKeyStyleOptions}
        value={value}
        note={nineKeyStyleNotes[value]}
        onChange={(touch_nine_key_style) => onChange({ touch_nine_key_style })}
      />
    );
  };
  return (
    <>
      {kind && (
        <GroupList>
          <div className="flex min-w-0 flex-col gap-3 px-4 py-4">
            {offered.length > 1 && (
              <div className="flex [&>span]:flex [&>span]:w-full [&>span>label]:flex-1">
                <Segmented
                  aria-label="键盘类型"
                  options={offered.map((value) => ({
                    value,
                    label: touchKeyLayoutKindTitles[value],
                  }))}
                  value={kind}
                  onChange={setChosen}
                />
              </div>
            )}
            {offered.map((panel) => (
              <SettingsSearchReveal
                key={panel}
                label={touchKeyLayoutKindTitles[panel]}
                hidden={panel !== kind}
                onReveal={() => setChosen(panel)}
              >
                <div className="flex min-w-0 flex-col gap-3" data-key-layout-panel={panel}>
                  <KeyLayoutPreview
                    kind={panel}
                    rows={keyLayoutPreviewRows(panel, preferences, scheme)}
                  />
                  {chips(panel)}
                </div>
              </SettingsSearchReveal>
            ))}
          </div>
          {groups.twentySix && (
            // 互换开关是卡片里说明之后的一行，只在 26 键分段显示；搜索命中它时切到 26 键。
            <SettingsSearchReveal
              label={touchKeyLayoutKindTitles.twentySix}
              hidden={kind !== "twentySix"}
              onReveal={() => setChosen("twentySix")}
            >
              <SwitchRow
                title={symbolFirstTitle}
                description={symbolFirstDescription}
                checked={preferences.touch_twenty_six_symbol_first === true}
                onChange={(touch_twenty_six_symbol_first) =>
                  onChange({ touch_twenty_six_symbol_first })
                }
              />
            </SettingsSearchReveal>
          )}
          {groups.nine && (
            // 九键左侧符号是 9 键分段卡片里说明之后的一行，与互换开关同一种写法；搜索命中它时切到 9 键。
            <SettingsSearchReveal
              label={touchKeyLayoutKindTitles.nine}
              hidden={kind !== "nine"}
              onReveal={() => setChosen("nine")}
            >
              <LinkRow
                title={nineKeySidebarTitle}
                value={nineKeySidebarSummary(nineKeySidebarSymbols(preferences))}
                onClick={() => openEditor("letter")}
              />
            </SettingsSearchReveal>
          )}
          {/* 中/英 换边对三类键盘都生效，每个分段都显示同一个开关，不挂在哪个分段下。 */}
          <SwitchRow
            title={languageKeyLeftTitle}
            description={languageKeyLeftDescription}
            checked={languageKeyLeft(preferences)}
            onChange={(left) => onChange({ touch_language_key_side: left ? "left" : "right" })}
          />
        </GroupList>
      )}
      {(groups.twentySix || groups.fourteen || groups.nine) && (
        <GroupList title={numberPadSpaceTitle}>
          <div className="flex min-w-0 flex-col gap-3 px-4 py-4">
            <StyleChips
              label={numberPadSpaceTitle}
              options={numberPadSpaceOptions}
              value={numberPadSpace(preferences)}
              note={numberPadSpaceNote}
              onChange={(touch_number_pad_space) => onChange({ touch_number_pad_space })}
            />
          </div>
          <LinkRow
            title={numberPadSidebarTitle}
            value={nineKeySidebarSummary(numberPadSidebarSymbols(preferences))}
            onClick={() => openEditor("digit")}
          />
        </GroupList>
      )}
      {(groups.twentySix || groups.fourteen) && (
        <GroupList>
          <SwitchRow
            title="顶部数字行"
            description={numberRowDescription}
            checked={preferences.touch_number_row === true}
            onChange={(touch_number_row) => onChange({ touch_number_row })}
          />
        </GroupList>
      )}
      {editing && (
        <NineKeySidebarSettings
          key={editing}
          kind={editing}
          symbols={
            editing === "letter"
              ? nineKeySidebarSymbols(preferences)
              : numberPadSidebarSymbols(preferences)
          }
          onSave={(symbols) => saveSidebar(editing, symbols)}
          onBack={closeEditor}
        />
      )}
    </>
  );
}
