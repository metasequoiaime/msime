import { useId, type ReactNode, type TextareaHTMLAttributes } from "react";
import { ControlledTextarea } from "../core/controlled-textarea";
import { InfoTip } from "../core/info-tip";
import * as settings from "./settings-style";

export interface SettingsTextareaFieldProps extends Omit<
  TextareaHTMLAttributes<HTMLTextAreaElement>,
  "value" | "onChange" | "aria-label"
> {
  label: ReactNode;
  /** 解释这一栏怎么填：收进标题旁的「?」，不常驻页面（见 docs/ARCHITECTURE.md「设置项说明收进「?」」）。 */
  description?: ReactNode;
  /** 状态、校验或错误提示：常驻标题旁，不收进「?」。 */
  detail?: ReactNode;
  ariaLabel: string;
  value: string;
  onChange: (value: string) => void;
}

/**
 * Shared labeled textarea field used by grouped settings forms.
 *
 * 外层是 `div` 而不是 `label`：标题旁的「?」是按钮，嵌进 `<label>` 会取代文本框成为它指向的控件（见 `InfoTip`）。文本框靠 `aria-label` 命名，说明和状态经 `aria-describedby` 读出。
 */
export function SettingsTextareaField({
  label,
  description,
  detail,
  ariaLabel,
  value,
  onChange,
  className = settings.promptInput,
  ...textareaProps
}: SettingsTextareaFieldProps) {
  const descriptionId = useId();
  const detailId = useId();
  const describedBy =
    [detail ? detailId : "", description !== undefined ? descriptionId : ""]
      .filter(Boolean)
      .join(" ") || undefined;
  return (
    <div className={settings.field}>
      <span>
        <span className="inline-flex items-center gap-1 align-middle">
          <span data-row-title="">{label}</span>
          {description !== undefined && (
            <InfoTip id={descriptionId} subject={label}>
              {description}
            </InfoTip>
          )}
        </span>
        {detail && <span id={detailId}> {detail}</span>}
      </span>
      <ControlledTextarea
        {...textareaProps}
        className={className}
        aria-label={ariaLabel}
        aria-describedby={describedBy}
        value={value}
        onChange={onChange}
      />
    </div>
  );
}
