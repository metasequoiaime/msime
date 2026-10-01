export interface EndpointInputProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

/** Shared URL input used by provider endpoint settings. */
export function EndpointInput({
  label,
  value,
  onChange,
  disabled = false,
  placeholder,
}: EndpointInputProps) {
  return (
    <input
      aria-label={label}
      type="url"
      value={value}
      disabled={disabled}
      placeholder={placeholder}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}
