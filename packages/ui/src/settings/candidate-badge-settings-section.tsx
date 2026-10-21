import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";
import type { Preferences } from "../core/preferences-model";

export type CandidateFixedBadgeStyle = NonNullable<Preferences["candidate_fixed_badge_style"]>;
export type CandidateAiBadgeStyle = NonNullable<Preferences["candidate_ai_badge_style"]>;

export interface CandidateBadgeSettingsSectionProps {
  fixedBadge: boolean;
  fixedBadgeStyle: CandidateFixedBadgeStyle;
  aiBadgeStyle: CandidateAiBadgeStyle;
  onFixedBadgeChange: (enabled: boolean) => void;
  onFixedBadgeStyleChange: (style: CandidateFixedBadgeStyle) => void;
  onAiBadgeStyleChange: (style: CandidateAiBadgeStyle) => void;
}

/**
 * 候选徽标：固定排位的开关与三档样式，AI 联想来源徽标的两档。
 *
 * 这两项在旧版 Windows 正式版里属于候选窗外观设置；新架构只有 macOS 宿主消费它们，
 * 所以调用方只在 macOS 渲染，别让其它平台看到一个不起作用的开关。
 */
export function CandidateBadgeSettingsSection({
  fixedBadge,
  fixedBadgeStyle,
  aiBadgeStyle,
  onFixedBadgeChange,
  onFixedBadgeStyleChange,
  onAiBadgeStyleChange,
}: CandidateBadgeSettingsSectionProps) {
  return (
    <>
      <SwitchRow
        title="固定候选徽标"
        description="为手动固定的候选词显示角标，便于辨认"
        checked={fixedBadge}
        onChange={onFixedBadgeChange}
      />
      <SelectRow
        title="固定候选徽标样式"
        aria-label="固定候选徽标样式"
        disabled={!fixedBadge}
        value={fixedBadgeStyle}
        onChange={(event) =>
          onFixedBadgeStyleChange(event.target.value as CandidateFixedBadgeStyle)
        }
      >
        <option value="paperclip">回形针 📎</option>
        <option value="pushpin">图钉 📌</option>
        <option value="dot">圆点 ·</option>
      </SelectRow>
      <SelectRow
        title="AI 联想候选徽标样式"
        description="AI 联想候选词末尾的来源角标：机器人 🤖 或小鲸鱼 🐳"
        aria-label="AI 联想候选徽标样式"
        value={aiBadgeStyle}
        onChange={(event) => onAiBadgeStyleChange(event.target.value as CandidateAiBadgeStyle)}
      >
        <option value="robot">机器人 🤖</option>
        <option value="whale">小鲸鱼 🐳</option>
      </SelectRow>
    </>
  );
}
