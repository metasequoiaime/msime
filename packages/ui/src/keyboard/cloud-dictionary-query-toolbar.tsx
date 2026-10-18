import { isComposingKeyEvent } from "../core/ime-key-event";
import type { ReactNode } from "react";
import * as cloud from "./cloud-panel-style";
import { CloudDictionaryKindSelect } from "./cloud-dictionary-kind-select";
import type { CloudDictionaryKind } from "./cloud-dictionary-kind-tabs";
import { ActionButton } from "../core/action-button";

export interface CloudDictionaryQueryToolbarProps {
  kind: CloudDictionaryKind;
  busy: boolean;
  queryLabel: string;
  queryAriaLabel: string;
  queryValue: string;
  queryButtonLabel: string;
  queryDisabled?: boolean;
  live?: boolean;
  placeholder?: string;
  inputClassName?: string;
  queryButtonClassName?: string;
  onKindChange: (kind: CloudDictionaryKind) => void;
  onQueryChange: (value: string) => void;
  onQuery: () => void;
  children?: ReactNode;
}

/** 云词库面板共用的类型选择、查询输入和提交控件。 */
export function CloudDictionaryQueryToolbar({
  kind,
  busy,
  queryLabel,
  queryAriaLabel,
  queryValue,
  queryButtonLabel,
  queryDisabled = false,
  live = false,
  placeholder,
  inputClassName,
  queryButtonClassName,
  onKindChange,
  onQueryChange,
  onQuery,
  children,
}: CloudDictionaryQueryToolbarProps) {
  return (
    <div className={live ? cloud.dictionaryLiveToolbar : cloud.dictionaryToolbar}>
      <label className={`${cloud.dictionaryField} ${cloud.dictionaryDesktopOnlyField}`}>
        词库
        <CloudDictionaryKindSelect value={kind} disabled={busy} onChange={onKindChange} />
      </label>
      <label className={`${cloud.dictionaryField} ${cloud.dictionarySearch} min-w-0`}>
        {queryLabel}
        <input
          {...(inputClassName ? { className: inputClassName } : {})}
          aria-label={queryAriaLabel}
          value={queryValue}
          onChange={(event) => onQueryChange(event.target.value)}
          onKeyDown={(event) => {
            if (
              live ||
              event.key !== "Enter" ||
              busy ||
              queryDisabled ||
              isComposingKeyEvent(event)
            )
              return;
            void onQuery();
          }}
          placeholder={placeholder}
        />
      </label>
      {!live && (
        <ActionButton
          action={onQuery}
          {...(queryButtonClassName ? { className: queryButtonClassName } : {})}
          disabled={busy || queryDisabled}
          label={queryButtonLabel}
        />
      )}
      {live ? <div className={cloud.dictionaryLiveActions}>{children}</div> : children}
    </div>
  );
}
