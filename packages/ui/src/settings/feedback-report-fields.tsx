import type { ReactNode } from "react";
import { FeedbackKindOptions } from "./feedback-kind-options";
import { SelectRow } from "./select-row";
import { SettingsTextareaField } from "./settings-textarea-field";
import { SettingsManagerBlock } from "./settings-manager-block";

export interface FeedbackReportFieldsProps {
  kind: string;
  detail: string;
  onKindChange: (value: string) => void;
  onDetailChange: (value: string) => void;
  children?: ReactNode;
}

/** Shared feedback type and description fields of the feedback page. */
export function FeedbackReportFields({
  kind,
  detail,
  onKindChange,
  onDetailChange,
  children,
}: FeedbackReportFieldsProps) {
  return (
    <>
      <SelectRow
        title="类型"
        aria-label="反馈类型"
        value={kind}
        onChange={(event) => onKindChange(event.target.value)}
      >
        <FeedbackKindOptions />
      </SelectRow>
      <SettingsManagerBlock>
        <SettingsTextareaField
          label="描述"
          ariaLabel="反馈描述"
          maxLength={4000}
          value={detail}
          onChange={onDetailChange}
          placeholder="发生了什么？如果和打字有关，写出输入方案、编码和期望结果。"
          rows={6}
        />
        {children}
      </SettingsManagerBlock>
    </>
  );
}
