import type { ReactNode } from "react";
import * as cloud from "./cloud-panel-style";
import { CloudDictionaryKindSelect } from "./cloud-dictionary-kind-select";
import type { CloudDictionaryKind } from "./cloud-dictionary-kind-tabs";

export interface CloudDictionaryQueryToolbarProps {
  kind: CloudDictionaryKind;
  busy: boolean;
  queryLabel: string;
  queryAriaLabel: string;
  queryValue: string;
  queryButtonLabel: string;
  queryDisabled?: boolean;
  placeholder?: string;
  inputClassName?: string;
  queryButtonClassName?: string;
  onKindChange: (kind: CloudDictionaryKind) => void;
  onQueryChange: (value: string) => void;
  onQuery: () => void;
  children?: ReactNode;
}

/** Shared kind, query, and submit controls for cloud dictionary panel toolbars. */
export function CloudDictionaryQueryToolbar({
  kind,
  busy,
  queryLabel,
  queryAriaLabel,
  queryValue,
  queryButtonLabel,
  queryDisabled = false,
  placeholder,
  inputClassName,
  queryButtonClassName,
  onKindChange,
  onQueryChange,
  onQuery,
  children,
}: CloudDictionaryQueryToolbarProps) {
  return (
    <div className={cloud.dictionaryToolbar}>
      <label className={`${cloud.dictionaryField} ${cloud.dictionaryDesktopOnlyField}`}>
        词库
        <CloudDictionaryKindSelect value={kind} disabled={busy} onChange={onKindChange} />
      </label>
      <label className={cloud.dictionarySearch}>
        {queryLabel}
        <input
          {...(inputClassName ? { className: inputClassName } : {})}
          aria-label={queryAriaLabel}
          value={queryValue}
          onChange={(event) => onQueryChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void onQuery();
          }}
          placeholder={placeholder}
        />
      </label>
      <button
        {...(queryButtonClassName ? { className: queryButtonClassName } : {})}
        type="button"
        onClick={onQuery}
        disabled={busy || queryDisabled}
      >
        {queryButtonLabel}
      </button>
      {children}
    </div>
  );
}
