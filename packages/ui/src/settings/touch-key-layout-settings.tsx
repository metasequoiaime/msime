import { useId } from "react";
import type { Preferences } from "../core/preferences-model";
import type { InputScheme } from "../core/input-scheme";
import type { PreviewKey } from "../keyboard/keyboard-layouts";
import { GroupList } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";
import {
  fourteenKeyBottomRows,
  fourteenKeyStyle,
  fourteenKeyStyleOptions,
  nineKeyBottomRows,
  nineKeyStyle,
  nineKeyStyleOptions,
  twentySixKeyBottomRows,
  twentySixKeyStyle,
  twentySixKeyStyleOptions,
} from "../keyboard/touch-key-layout-styles";

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

/** 一组单选：每个选项是一行，名字在上、它的底行示意图在下。 */
function StyleChoices<T extends string>({
  title,
  options,
  rows,
  value,
  onChange,
}: {
  title: string;
  options: readonly { value: T; label: string }[];
  rows: Record<T, PreviewKey[]>;
  value: T;
  onChange: (value: T) => void;
}) {
  const name = useId();
  return (
    <GroupList title={title}>
      <div role="radiogroup" aria-label={title} className="flex flex-col">
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
            <BottomRowPreview row={rows[option.value]} />
          </label>
        ))}
      </div>
    </GroupList>
  );
}

/**
 * 「按键布局」子页的内容：26 键、14 键与 18 键、9 键三组底行样式各选一种，每个选项下画出它的底行；最后是「顶部数字行」开关。写的是共享偏好 `touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style` 和 `touch_number_row`，键盘开着时当场换排法。
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
  return (
    <>
      {groups.twentySix && (
        <StyleChoices
          title="26 键"
          options={twentySixKeyStyleOptions}
          rows={twentySixKeyBottomRows}
          value={twentySixKeyStyle(preferences)}
          onChange={(touch_twenty_six_key_style) => onChange({ touch_twenty_six_key_style })}
        />
      )}
      {groups.fourteen && (
        <StyleChoices
          title="14 键与 18 键"
          options={fourteenKeyStyleOptions}
          rows={fourteenKeyBottomRows}
          value={fourteenKeyStyle(preferences)}
          onChange={(touch_fourteen_key_style) => onChange({ touch_fourteen_key_style })}
        />
      )}
      {groups.nine && (
        <StyleChoices
          title="9 键"
          options={nineKeyStyleOptions}
          rows={nineKeyBottomRows}
          value={nineKeyStyle(preferences)}
          onChange={(touch_nine_key_style) => onChange({ touch_nine_key_style })}
        />
      )}
      {(groups.twentySix || groups.fourteen) && (
        <GroupList>
          <SwitchRow
            title="顶部数字行"
            description="26 键、14 键和 18 键的字母上方加一行 1 到 0，键盘高一行；手机横屏时不显示"
            checked={preferences.touch_number_row === true}
            onChange={(touch_number_row) => onChange({ touch_number_row })}
          />
        </GroupList>
      )}
    </>
  );
}
