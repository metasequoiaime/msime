import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import { SelectRow } from "./select-row";

export interface AiTranslationSettingsSectionProps {
  grouped?: boolean;
  /** 「当前服务商与模型」一行的展示值；未配置时为空。 */
  summary: string;
  /** AI 辅助的接口地址、Token、模型是否已可用；不可用时显示提示而不是静默不翻译。 */
  ready: boolean;
  credentialTest?: ReactNode;
}

/** 「翻译服务」选「AI 辅助」时的设置区块：复用「AI 与云联想」页的服务商、接口地址、Token 与模型（与 Windows 设置页同名区块一致）。 */
export function AiTranslationSettingsSection({
  grouped = false,
  summary,
  ready,
  credentialTest,
}: AiTranslationSettingsSectionProps) {
  const content = (
    <>
      <SelectRow
        title="当前服务商与模型"
        detail={ready ? undefined : "请先在「AI 与云联想」页面填写接口地址、Token 和模型"}
        value={summary || "未配置"}
        disabled
      >
        <option value="">{summary || "未配置"}</option>
      </SelectRow>
      {credentialTest}
    </>
  );
  return grouped ? <GroupList title="复用 AI 辅助配置">{content}</GroupList> : content;
}
