import { SettingToggle } from "./setting-toggle";

export interface CloudCandidatesSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Online candidate lookup switch shared by input settings hosts. */
export function CloudCandidatesSection({ value, onChange }: CloudCandidatesSectionProps) {
  return (
    <SettingToggle
      label="云候选"
      description="向在线服务请求额外候选"
      ariaLabel="云候选"
      checked={value ?? true}
      onChange={onChange}
    />
  );
}
