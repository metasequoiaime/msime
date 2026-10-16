export type PanelActivationMode = "copy" | "input";

export interface PanelActivationButtonsProps {
  mode: PanelActivationMode;
  onModeChange: (mode: PanelActivationMode) => void;
  inputAvailable: boolean;
  disabled?: boolean;
  copyDisabled?: boolean;
  inputLabel?: string;
  buttonClassName?: (selected: boolean) => string;
}

/** 面板共用的复制、输入模式按钮；模式、能力回退与分组外观由面板持有。 */
export function PanelActivationButtons({
  mode,
  onModeChange,
  inputAvailable,
  disabled = false,
  copyDisabled = false,
  inputLabel = "输入",
  buttonClassName,
}: PanelActivationButtonsProps) {
  return (
    <>
      <button
        type="button"
        className={buttonClassName?.(mode === "copy")}
        aria-pressed={mode === "copy"}
        disabled={disabled || copyDisabled}
        onClick={() => onModeChange("copy")}
      >
        复制
      </button>
      {inputAvailable && (
        <button
          type="button"
          className={buttonClassName?.(mode === "input")}
          aria-pressed={mode === "input"}
          disabled={disabled}
          onClick={() => onModeChange("input")}
        >
          {inputLabel}
        </button>
      )}
    </>
  );
}
