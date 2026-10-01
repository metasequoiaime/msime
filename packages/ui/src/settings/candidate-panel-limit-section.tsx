import { candidatePanelLimitNotes } from "./settings-options";
import * as settings from "./settings-style";

export type CandidatePanelLimit = "gnome_shell" | "fcitx_theme" | "kimpanel";

export interface CandidatePanelLimitSectionProps {
  limit: CandidatePanelLimit;
}

/** Explains which Linux desktop surface owns the candidate panel appearance: a note under the 候选窗口 page's preview and at the top of the 主题 page, the two pages whose settings that surface ignores. */
export function CandidatePanelLimitSection({ limit }: CandidatePanelLimitSectionProps) {
  return <p className={settings.groupNote}>{candidatePanelLimitNotes[limit]}</p>;
}
