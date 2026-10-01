import { SwitchRow } from "./switch-row";

export interface CandidateEnglishGlossSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** 离线英文释义开关，供提供候选注释的宿主使用：「标点与翻译」页「多语言与释义」组中的一行。 */
export function CandidateEnglishGlossSection({
  value,
  onChange,
}: CandidateEnglishGlossSectionProps) {
  return (
    <SwitchRow
      title="显示英文释义"
      description="在候选词后面标出它的英文意思，中文候选给英文、英文候选给中文。释义来自随键盘打包的离线词库，不联网。"
      checked={value ?? false}
      onChange={onChange}
    />
  );
}
