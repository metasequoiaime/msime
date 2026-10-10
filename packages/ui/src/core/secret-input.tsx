import { useId, useState } from "react";
import * as style from "./platform-controls-style";

/** A credential field whose value can be revealed to check a pasted key. */
export function SecretInput({
  label,
  value,
  disabled,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  disabled?: boolean;
  onChange: (value: string) => void;
  placeholder?: string;
}) {
  const [revealed, setRevealed] = useState(false);
  const describedBy = useId();
  return (
    <span className={`secret-input ${style.secretInput}`}>
      <input
        className={style.textInput}
        aria-label={label}
        type={revealed ? "text" : "password"}
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        aria-describedby={describedBy}
        onChange={(event) => onChange(event.target.value)}
      />
      <button
        type="button"
        className={`secret-input-toggle ${style.secretToggle}`}
        id={describedBy}
        aria-pressed={revealed}
        aria-label={revealed ? `隐藏${label}` : `显示${label}`}
        title={revealed ? "隐藏" : "显示"}
        disabled={disabled}
        onClick={() => setRevealed((current) => !current)}
      >
        {revealed ? "隐藏" : "显示"}
      </button>
    </span>
  );
}
