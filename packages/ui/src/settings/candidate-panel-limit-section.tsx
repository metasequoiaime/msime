import { candidatePanelLimitNotes } from "./settings-options";
import * as settings from "./settings-style";

export type CandidatePanelLimit = "gnome_shell" | "fcitx_theme" | "kimpanel";

export interface CandidatePanelLimitSectionProps {
  limit: CandidatePanelLimit;
}

/** 说明候选面板外观由 Linux 桌面的哪个组件接管：显示在「候选窗口」页的预览下方和「主题」页顶部，这两页的设置都会被该组件忽略。 */
export function CandidatePanelLimitSection({ limit }: CandidatePanelLimitSectionProps) {
  return <p className={settings.groupNote}>{candidatePanelLimitNotes[limit]}</p>;
}
