import { useId, type ReactNode, type TextareaHTMLAttributes } from "react";
import { ControlledTextarea } from "../core/controlled-textarea";
import { InfoTip } from "../core/info-tip";
import * as settings from "./settings-style";

export interface SettingsTextareaFieldProps extends Omit<
  TextareaHTMLAttributes<HTMLTextAreaElement>,
  "value" | "onChange" | "aria-label"
> {
  label: ReactNode;
  /** 这个字段填什么、怎么填：收进标签旁的「?」，不常驻页面（见 docs/ARCHITECTURE.md「设置项说明收进「?」」）。 */
  description?: ReactNode;
  /** 校验问题和状态：常驻标签后面。 */
  detail?: ReactNode;
  ariaLabel: string;
  value: string;
  onChange: (value: string) => void;
}

/**
 * Shared labeled textarea field used by grouped settings forms.
 *
 * 标签不包住文本框：包住时「?」只能是悬停才出气泡的图标（`<label>` 里不能放按钮），手机上就看不到说明。标签用 `htmlFor` 指向文本框，「?」放在标签外面，文本框经 `aria-describedby` 读到说明和状态。
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
  const ownId = useId();
  const fieldId = textareaProps.id ?? ownId;
  const descriptionId = useId();
  const detailId = useId();
  const describedBy =
    [
      textareaProps["aria-describedby"],
      detail !== undefined && detail !== null && detail !== "" ? detailId : "",
      description !== undefined ? descriptionId : "",
    ]
      .filter(Boolean)
      .join(" ") || undefined;
  return (
    <div className={settings.field}>
      <span>
        <label htmlFor={fieldId} data-row-title="">
          {label}
        </label>
        {description !== undefined && (
          <InfoTip id={descriptionId} subject={label} className="ml-1 align-middle">
            {description}
          </InfoTip>
        )}
        {detail !== undefined && detail !== null && detail !== "" && (
          <>
            {" "}
            <span id={detailId}>{detail}</span>
          </>
        )}
      </span>
      <ControlledTextarea
        {...textareaProps}
        id={fieldId}
        className={className}
        aria-label={ariaLabel}
        aria-describedby={describedBy}
        value={value}
        onChange={onChange}
      />
    </div>
  );
}
