import { SettingToggle } from "./setting-toggle";

export interface CandidateEnglishGlossSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Offline English gloss switch for hosts that expose candidate annotations. */
export function CandidateEnglishGlossSection({
  value,
  onChange,
}: CandidateEnglishGlossSectionProps) {
  return (
    <SettingToggle
      label="显示英文释义"
      description="在候选词后面标出它的英文意思，中文候选给英文、英文候选给中文。释义来自随键盘打包的离线词库，不联网。"
      ariaLabel="显示英文释义"
      checked={value ?? false}
      onChange={onChange}
    />
  );
}
