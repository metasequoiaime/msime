import { candidatePanelLimitNotes } from "./settings-options";

export type CandidatePanelLimit = "gnome_shell" | "fcitx_theme" | "kimpanel";

export interface CandidatePanelLimitSectionProps {
  limit: CandidatePanelLimit;
}

/** Explains which Linux desktop surface owns the candidate panel appearance. */
export function CandidatePanelLimitSection({ limit }: CandidatePanelLimitSectionProps) {
  return (
    <div className="section">
      <small>{candidatePanelLimitNotes[limit]}</small>
    </div>
  );
}
