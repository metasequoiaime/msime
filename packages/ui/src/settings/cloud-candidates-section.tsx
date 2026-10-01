import { SwitchRow } from "./switch-row";

export interface CloudCandidatesSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Online candidate lookup switch shared by input settings hosts: one row of the 输出 group. */
export function CloudCandidatesSection({ value, onChange }: CloudCandidatesSectionProps) {
  return (
    <SwitchRow
      title="云候选"
      description="向在线服务请求额外候选"
      checked={value ?? true}
      onChange={onChange}
    />
  );
}
