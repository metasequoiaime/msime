import type { ReactNode, TextareaHTMLAttributes } from "react";

export interface SettingTextareaControlProps extends Omit<
  TextareaHTMLAttributes<HTMLTextAreaElement>,
  "value" | "onChange" | "aria-label"
> {
  label: ReactNode;
  description?: ReactNode;
  ariaLabel: string;
  value: string;
  onChange: (value: string) => void;
  /** Legacy feedback uses a div title because its surrounding section is already a form group. */
  titleElement?: "label" | "div";
}

/** Shared labeled textarea control used by stacked and embedded legacy settings. */
export function SettingTextareaControl({
  label,
  description,
  ariaLabel,
  value,
  onChange,
  titleElement = "label",
  ...textareaProps
}: SettingTextareaControlProps) {
  const Title = titleElement;
  return (
    <>
      <Title className="section-title">
        {label}
        {description !== undefined && <small>{description}</small>}
      </Title>
      <textarea
        {...textareaProps}
        aria-label={ariaLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </>
  );
}

export interface SettingTextareaProps extends SettingTextareaControlProps {
  sectionClassName?: string;
  children?: ReactNode;
}

/** Shared stacked textarea field used by prompt and other multiline settings. */
export function SettingTextarea({
  sectionClassName = "section",
  children,
  ...controlProps
}: SettingTextareaProps) {
  return (
    <div className={sectionClassName}>
      <SettingTextareaControl {...controlProps} />
      {children}
    </div>
  );
}
