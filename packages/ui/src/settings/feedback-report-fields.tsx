import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import * as settings from "./settings-style";
import { FeedbackKindOptions } from "./feedback-kind-options";
import { SelectSettingField } from "./select-setting-field";
import { SelectRow } from "./select-row";

export interface FeedbackReportFieldsProps {
  grouped?: boolean;
  kind: string;
  detail: string;
  onKindChange: (value: string) => void;
  onDetailChange: (value: string) => void;
  children?: ReactNode;
}

/** Shared feedback type and description fields for grouped and legacy settings hosts. */
export function FeedbackReportFields({
  grouped = false,
  kind,
  detail,
  onKindChange,
  onDetailChange,
  children,
}: FeedbackReportFieldsProps) {
  const controls = grouped ? (
    <>
      <SelectRow
        title="类型"
        aria-label="反馈类型"
        value={kind}
        onChange={(event) => onKindChange(event.target.value)}
      >
        <FeedbackKindOptions />
      </SelectRow>
      <div className={settings.managerBlock}>
        <label className={settings.field}>
          <span data-row-title="">描述</span>
          <textarea
            aria-label="反馈描述"
            className={settings.promptInput}
            maxLength={4000}
            value={detail}
            onChange={(event) => onDetailChange(event.target.value)}
            placeholder="发生了什么？如果和打字有关，写出输入方案、编码和期望结果。"
            rows={6}
          />
        </label>
        {children}
      </div>
    </>
  ) : (
    <>
      <SelectSettingField label="类型" inputLabel="反馈类型" value={kind} onChange={onKindChange}>
        <FeedbackKindOptions />
      </SelectSettingField>
      <label className="section-title">
        描述
        <textarea
          aria-label="反馈描述"
          maxLength={4000}
          value={detail}
          onChange={(event) => onDetailChange(event.target.value)}
          placeholder="发生了什么？如果和打字有关，写出输入方案、编码和期望结果。"
          rows={6}
        />
      </label>
      {children}
    </>
  );

  return controls;
}
