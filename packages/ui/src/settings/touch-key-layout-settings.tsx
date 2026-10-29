import { useId, type ReactNode } from "react";
import type { Preferences } from "../core/preferences-model";
import type { InputScheme } from "../core/input-scheme";
import type { PreviewKey } from "../keyboard/keyboard-layouts";
import { GroupList } from "../core/platform-controls";
import { SettingsGroupNote } from "./settings-group-note";
import { SwitchRow } from "./switch-row";
import {
  fourteenKeyBottomRows,
  fourteenKeyStyle,
  fourteenKeyStyleNote,
  fourteenKeyStyleOptions,
  nineKeyBottomRows,
  nineKeyStyle,
  nineKeyStyleNote,
  nineKeyStyleOptions,
  nineKeyTallReturnPreview,
  numberPadSpace,
  numberPadSpaceNote,
  numberPadSpaceOptions,
  numberPadSpaceTitle,
  numberRowDescription,
  symbolFirstDescription,
  symbolFirstTitle,
  twentySixKeyBottomRows,
  type FramePreviewKey,
  twentySixKeyStyle,
  twentySixKeyStyleNote,
  twentySixKeyStyleOptions,
  withSymbolFirst,
} from "../keyboard/touch-key-layout-styles";

/** 打开「符」与「123」互换时 26 键一组的示意图：每种样式的底行都把 符 挪到 123 左边（没有 符 的样式不变）。 */
const twentySixKeyRowsSymbolFirst = Object.fromEntries(
  Object.entries(twentySixKeyBottomRows).map(([style, row]) => [style, withSymbolFirst(row, true)]),
) as typeof twentySixKeyBottomRows;

/** 「按键布局」子页上哪几组画出来：版本没有的键盘类型不列它的组。 */
export type TouchKeyLayoutGroups = {
  twentySix: boolean;
  fourteen: boolean;
  nine: boolean;
};

/**
 * 按版本提供的方案决定列哪几组：中文 26 键来自全拼、双拼、五笔和粤拼；14 键与 18 键来自全拼、双拼（14/18 键）和五笔（14 键）；九键来自全拼九键、九键双拼和笔画。只有日文、韩文等非中文方案的版本一组都没有。
 */
export function touchKeyLayoutGroups(schemes: readonly InputScheme[]): TouchKeyLayoutGroups {
  const has = (...wanted: InputScheme[]) => wanted.some((scheme) => schemes.includes(scheme));
  return {
    twentySix: has("quanpin", "shuangpin", "wubi", "cantonese"),
    fourteen: has("quanpin", "shuangpin", "wubi"),
    nine: has("quanpin", "shuangpin", "stroke"),
  };
}

/** 一种样式底行的小示意图：按份额铺满一行，键面只是提示，不可点，读屏跳过它（选项本身已有名字）。 */
function BottomRowPreview({ row }: { row: readonly PreviewKey[] }) {
  return (
    <span className="flex w-full gap-[3px]" aria-hidden="true" data-key-layout-preview="">
      {row.map((key, index) => (
        <span
          key={`${index}-${key.label}`}
          className="flex h-6 min-w-0 items-center justify-center overflow-hidden rounded-[4px] border text-[11px] leading-none whitespace-nowrap [border-color:var(--p-row-divider)] [color:var(--p-sub)]"
          style={{ flex: `${key.weight} 1 0%` }}
          data-weight={key.weight}
        >
          {key.label}
        </span>
      ))}
    </span>
  );
}

/** 跨两行的回车往下伸到下一行的占位上：高度是两行加一道行距，下边距收回一行，所在行的高度不变。 */
const PREVIEW_ROW_PX = 24;
const PREVIEW_GAP_PX = 3;

/** 一种样式里不止底行变了时（九键「右侧大回车」）的示意图：画网格最后一行和底行，`rowSpan` 为 2 的键跨这两行。每一行与 `BottomRowPreview` 同样按份额铺满。 */
function FramePreview({ rows }: { rows: readonly (readonly FramePreviewKey[])[] }) {
  return (
    <span
      className="flex w-full flex-col"
      style={{ gap: PREVIEW_GAP_PX }}
      aria-hidden="true"
      data-key-layout-frame=""
    >
      {rows.map((row, rowIndex) => (
        <span
          key={rowIndex}
          className="flex w-full items-start"
          style={{ gap: PREVIEW_GAP_PX }}
          data-key-layout-preview=""
        >
          {row.map((key, index) => {
            const span = key.rowSpan ?? 1;
            return (
              <span
                key={`${index}-${key.label}`}
                className={`flex min-w-0 items-center justify-center overflow-hidden rounded-[4px] text-[11px] leading-none whitespace-nowrap [color:var(--p-sub)] ${key.label ? "border [border-color:var(--p-row-divider)]" : ""}`}
                style={{
                  flex: `${key.weight} 1 0%`,
                  height: span * PREVIEW_ROW_PX + (span - 1) * PREVIEW_GAP_PX,
                  marginBottom: -(span - 1) * (PREVIEW_ROW_PX + PREVIEW_GAP_PX),
                }}
                data-weight={key.weight}
                data-row-span={span}
              >
                {key.label}
              </span>
            );
          })}
        </span>
      ))}
    </span>
  );
}

/** 一组单选：每个选项是一行，名字在上、它的底行示意图在下（`frames` 里有的样式画多行示意，没有 `rows` 时不画示意）；`note` 是组末的说明，读屏把它当作这组单选的描述；`children` 画在选项之后、说明之前（26 键一组的「符」与「123」互换开关）。 */
function StyleChoices<T extends string>({
  title,
  options,
  rows,
  frames,
  note,
  value,
  onChange,
  children,
}: {
  title: string;
  options: readonly { value: T; label: string }[];
  rows?: Record<T, PreviewKey[]>;
  frames?: Partial<Record<T, FramePreviewKey[][]>>;
  note: string;
  value: T;
  onChange: (value: T) => void;
  children?: ReactNode;
}) {
  const name = useId();
  const noteId = useId();
  return (
    <GroupList title={title}>
      <div role="radiogroup" aria-label={title} aria-describedby={noteId} className="flex flex-col">
        {options.map((option) => (
          <label
            key={option.value}
            className="flex cursor-pointer flex-col gap-2 border-b px-4 py-3 [border-color:var(--p-row-divider)] last:border-b-0 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-accent"
            data-settings-search-label={option.label}
          >
            <span className="flex items-center gap-2.5 [font-size:var(--p-row-fs)]">
              <input
                type="radio"
                name={name}
                value={option.value}
                checked={value === option.value}
                onChange={() => onChange(option.value)}
                className="m-0 shrink-0 accent-accent"
              />
              {option.label}
            </span>
            {frames?.[option.value] ? (
              <FramePreview rows={frames[option.value] ?? []} />
            ) : rows ? (
              <BottomRowPreview row={rows[option.value]} />
            ) : null}
          </label>
        ))}
      </div>
      {children}
      <SettingsGroupNote>
        <span id={noteId}>{note}</span>
      </SettingsGroupNote>
    </GroupList>
  );
}

/**
 * 「按键布局」子页的内容：26 键、14 键与 18 键、9 键三组底行样式各选一种，每个选项下画出它的底行（九键「右侧大回车」画网格最后一行和底行，回车跨两行），每组末尾一句说明（与 Android、iOS 逐字相同）；26 键一组里另有「26 键「符」与「123」互换」开关，打开时这组的示意图也把 符 画在 123 左边；接着是「数字键盘」一组（空格在右列、0 右边或 0 左边，没选过时勾鸿蒙原来的「空格在 0 左边」）；最后是「顶部数字行」开关。写的是共享偏好 `touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`、`touch_twenty_six_symbol_first`、`touch_number_pad_space` 和 `touch_number_row`，键盘开着时当场换排法。
 */
export function TouchKeyLayoutSettings({
  preferences,
  groups,
  onChange,
}: {
  preferences: Preferences;
  groups: TouchKeyLayoutGroups;
  onChange: (patch: Partial<Preferences>) => void;
}) {
  const symbolFirst = preferences.touch_twenty_six_symbol_first === true;
  return (
    <>
      {groups.twentySix && (
        <StyleChoices
          title="26 键"
          options={twentySixKeyStyleOptions}
          rows={symbolFirst ? twentySixKeyRowsSymbolFirst : twentySixKeyBottomRows}
          note={twentySixKeyStyleNote}
          value={twentySixKeyStyle(preferences)}
          onChange={(touch_twenty_six_key_style) => onChange({ touch_twenty_six_key_style })}
        >
          <SwitchRow
            title={symbolFirstTitle}
            description={symbolFirstDescription}
            checked={symbolFirst}
            onChange={(touch_twenty_six_symbol_first) => onChange({ touch_twenty_six_symbol_first })}
          />
        </StyleChoices>
      )}
      {groups.fourteen && (
        <StyleChoices
          title="14 键与 18 键"
          options={fourteenKeyStyleOptions}
          rows={fourteenKeyBottomRows}
          note={fourteenKeyStyleNote}
          value={fourteenKeyStyle(preferences)}
          onChange={(touch_fourteen_key_style) => onChange({ touch_fourteen_key_style })}
        />
      )}
      {groups.nine && (
        <StyleChoices
          title="9 键"
          options={nineKeyStyleOptions}
          rows={nineKeyBottomRows}
          frames={{ tall_return: nineKeyTallReturnPreview }}
          note={nineKeyStyleNote}
          value={nineKeyStyle(preferences)}
          onChange={(touch_nine_key_style) => onChange({ touch_nine_key_style })}
        />
      )}
      {(groups.twentySix || groups.fourteen || groups.nine) && (
        <StyleChoices
          title={numberPadSpaceTitle}
          options={numberPadSpaceOptions}
          note={numberPadSpaceNote}
          value={numberPadSpace(preferences)}
          onChange={(touch_number_pad_space) => onChange({ touch_number_pad_space })}
        />
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
    </>
  );
}
