import type { ReactNode } from "react";

export interface LegacyRadioGroupOption<T extends string> {
  value: T;
  label: ReactNode;
  disabled?: boolean;
}

export interface LegacyRadioGroupProps<T extends string> {
  title: ReactNode;
  titleId: string;
  name: string;
  options: readonly LegacyRadioGroupOption<T>[];
  value: T;
  onChange?: (value: T) => void;
  description?: ReactNode;
  descriptionClassName?: string;
  hidden?: boolean;
  showDividers?: boolean;
}

/** Shared legacy radio group used by the old and grouped input settings surfaces. */
export function LegacyRadioGroup<T extends string>({
  title,
  titleId,
  name,
  options,
  value,
  onChange,
  description,
  descriptionClassName,
  hidden = false,
  showDividers = false,
}: LegacyRadioGroupProps<T>) {
  return (
    <div className="section" role="group" aria-labelledby={titleId} hidden={hidden}>
      <div className="section-title" id={titleId}>
        {title}
      </div>
      <div className="input-option-content">
        {options.map((option, index) => {
          const radio = (
            <label className="radio-option" key={option.value}>
              <input
                type="radio"
                name={name}
                value={option.value}
                checked={value === option.value}
                disabled={option.disabled}
                readOnly={!onChange}
                onChange={() => onChange?.(option.value)}
              />
              <span>{option.label}</span>
            </label>
          );

          return showDividers ? (
            <div className="input-option-item" key={option.value}>
              {index > 0 && <div className="input-option-divider" />}
              {radio}
            </div>
          ) : (
            radio
          );
        })}
      </div>
      {description !== undefined && (
        <div
          className={`input-setting-description${descriptionClassName ? ` ${descriptionClassName}` : ""}`}
        >
          {description}
        </div>
      )}
    </div>
  );
}
