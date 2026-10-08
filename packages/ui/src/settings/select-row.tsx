import {
  Children,
  Fragment,
  isValidElement,
  useRef,
  useState,
  type ReactNode,
  type SelectHTMLAttributes,
} from "react";
import type { SheetOption } from "../core/action-sheet";
import { PickerRow, Row, Select, textOf } from "../core/platform-controls";
import { useOptionalSettingsForm } from "./settings-form-context";

export interface SelectRowProps extends Omit<
  SelectHTMLAttributes<HTMLSelectElement>,
  "title" | "children"
> {
  title: ReactNode;
  description?: ReactNode;
  hidden?: boolean;
  children?: ReactNode;
}

/** 呈现原生 select 控件的设置行；在鸿蒙手机上整行改为打开选择面板。 */
export function SelectRow(props: SelectRowProps) {
  const form = useOptionalSettingsForm();
  if (form?.settingsPlatform === "harmony") return <SheetSelectRow {...props} />;
  const { title, description, hidden, children, ...selectProps } = props;
  return (
    <Row title={title} description={description} hidden={hidden}>
      <Select {...selectProps}>{children}</Select>
    </Row>
  );
}

/** 把 select 的 `<option>` 和 `<optgroup>` 子元素转成面板选项，`optgroup` 变为子菜单。只识别直接的 `<option>`/`<optgroup>`/Fragment 子元素，组件包装的选项会被忽略，调用方需传入返回 Fragment 的普通函数结果。 */
function sheetOptions(children: ReactNode): SheetOption[] {
  return Children.toArray(children).flatMap((child): SheetOption[] => {
    if (!isValidElement<{ children?: ReactNode }>(child)) return [];
    if (child.type === Fragment) return sheetOptions(child.props.children);
    if (child.type === "optgroup") {
      const group = child.props as { label?: string; disabled?: boolean; children?: ReactNode };
      const label = group.label ?? "";
      return [
        {
          value: `optgroup:${label}`,
          label,
          disabled: group.disabled,
          submenu: { title: label, options: sheetOptions(group.children) },
        },
      ];
    }
    if (child.type !== "option") return [];
    const option = child.props as {
      value?: string | number | readonly string[];
      disabled?: boolean;
      hidden?: boolean;
      children?: ReactNode;
    };
    // 页面隐藏的选项（只为让 select 能显示而保留的占位项）不提供。
    if (option.hidden) return [];
    const label = textOf(option.children);
    const value = option.value === undefined ? label : String(option.value);
    return [{ value, label, disabled: option.disabled }];
  });
}

/** 把选中的选项及包含它的子菜单标为已选。 */
function markSelected(options: readonly SheetOption[], value: string): SheetOption[] {
  return options.map((option) => {
    if (!option.submenu) return { ...option, selected: option.value === value };
    const nested = markSelected(option.submenu.options, value);
    return {
      ...option,
      selected: nested.some((item) => item.selected),
      submenu: { ...option.submenu, options: nested },
    };
  });
}

function firstChoice(options: readonly SheetOption[]): SheetOption | undefined {
  for (const option of options) {
    if (option.submenu) {
      const nested = firstChoice(option.submenu.options);
      if (nested) return nested;
    } else return option;
  }
  return undefined;
}

function findChoice(options: readonly SheetOption[], value: string): SheetOption | undefined {
  for (const option of options) {
    const found = option.submenu ? findChoice(option.submenu.options, value) : undefined;
    if (found) return found;
    if (!option.submenu && option.value === value) return option;
  }
  return undefined;
}

function SheetSelectRow({
  title,
  description,
  hidden,
  children,
  value,
  defaultValue,
  onChange,
  disabled,
  ...selectProps
}: SelectRowProps) {
  const select = useRef<HTMLSelectElement>(null);
  // 只有非受控的 select 需要这个：受控的显示 `value`，由其所有者更新。
  const [uncontrolled, setUncontrolled] = useState<string | undefined>(
    defaultValue === undefined ? undefined : String(defaultValue),
  );
  const current = value === undefined ? uncontrolled : String(value);
  const choices = sheetOptions(children);
  // 与原生元素一样，值不匹配任何选项的 select 显示第一个选项。
  const shown =
    (current === undefined ? undefined : findChoice(choices, current)) ?? firstChoice(choices);
  const options = shown ? markSelected(choices, shown.value) : choices;
  return (
    <PickerRow
      title={title}
      description={description}
      hidden={hidden}
      disabled={disabled}
      valueLabel={shown?.label ?? ""}
      options={options}
      onSelect={(next) => {
        const element = select.current;
        if (!element || element.value === next) return;
        // 写入原生 select 并以真实的 change 事件通告，让每个调用方的 `onChange(event)` 读取 `event.target.value` 的方式与其他平台完全一致。
        element.value = next;
        element.dispatchEvent(new Event("change", { bubbles: true }));
      }}
      control={(titleId) => (
        <Select
          {...selectProps}
          ref={select}
          value={value}
          defaultValue={defaultValue}
          disabled={disabled}
          aria-labelledby={
            selectProps["aria-labelledby"] ?? (selectProps["aria-label"] ? undefined : titleId)
          }
          tabIndex={-1}
          onChange={(event) => {
            if (value === undefined) setUncontrolled(event.currentTarget.value);
            onChange?.(event);
          }}
        >
          {children}
        </Select>
      )}
    />
  );
}
