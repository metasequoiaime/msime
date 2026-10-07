import { SwitchRow } from "./switch-row";

export interface CandidateAppLogoSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** 「显示水杉 logo」开关，供按 `show_app_logo` 画 logo 的宿主使用（`HostCapabilities::app_logo`）：「候选窗口」页「布局」组中的一行。新装默认关闭，所以缺值读成关。 */
export function CandidateAppLogoSection({ value, onChange }: CandidateAppLogoSectionProps) {
  return (
    <SwitchRow
      title="显示水杉 logo"
      description="在候选窗和悬浮工具栏左端显示水杉图标。"
      checked={value === true}
      onChange={onChange}
    />
  );
}
