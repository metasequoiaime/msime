import { useState, type ReactNode } from "react";
import { SelectRow } from "./select-row";
import { TextInputRow } from "./text-input-row";

/** 「自定义…」的选项值：已知模型里的空名字都滤掉了，所以空值不会和哪个模型撞上。 */
const CUSTOM = "";

export interface ModelPickerRowsProps {
  title: ReactNode;
  description?: ReactNode;
  /** 下拉框的无障碍名称；文本框用它加「（自定义）」。 */
  label: string;
  /** 已知的模型：服务商预置的，以及从服务获取的。重复的只列一次。 */
  models: readonly string[];
  model: string;
  onChange: (model: string) => void;
}

/**
 * 模型只用一个控件选：下拉里是已知的模型，末项「自定义…」；选了它，或者当前模型不在列表里，下面多一行文本框自己填写。没有已知模型时只有文本框。
 *
 * 以前预置模型、模型文本框、已获取模型各占一行，三处显示同一个值。在文本框里打字时即使打出列表里有的名字，文本框也不收起，免得打到一半控件消失；列表换了（换服务商、获取到新列表）才回到按当前模型判断。
 */
export function ModelPickerRows({
  title,
  description,
  label,
  models,
  model,
  onChange,
}: ModelPickerRowsProps) {
  const known = [...new Set(models.filter((entry) => entry.length > 0))];
  const listKey = known.join("\n");
  const [requested, setRequested] = useState({ listKey, custom: false });
  const custom = (requested.listKey === listKey && requested.custom) || !known.includes(model);
  const requestCustom = (value: boolean) => setRequested({ listKey, custom: value });
  if (known.length === 0)
    return (
      <TextInputRow
        title={title}
        description={description}
        label={label}
        value={model}
        onChange={onChange}
      />
    );
  return (
    <>
      <SelectRow
        title={title}
        description={description}
        aria-label={label}
        value={custom ? CUSTOM : model}
        onChange={(event) => {
          const value = event.target.value;
          requestCustom(value === CUSTOM);
          if (value !== CUSTOM) onChange(value);
        }}
      >
        {known.map((entry) => (
          <option key={entry} value={entry}>
            {entry}
          </option>
        ))}
        <option value={CUSTOM}>自定义…</option>
      </SelectRow>
      {custom && (
        <TextInputRow
          title="自定义模型"
          label={`${label}（自定义）`}
          value={model}
          onChange={(value) => {
            requestCustom(true);
            onChange(value);
          }}
        />
      )}
    </>
  );
}
