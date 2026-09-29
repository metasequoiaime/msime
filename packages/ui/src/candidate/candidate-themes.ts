export type CandidateOrientation = "horizontal" | "vertical";
export type CandidateAppearance = "light" | "dark";

/** Files copied from the upstream WebView2 candidate renderer. */
export function candidateTemplate(
  orientation: CandidateOrientation,
  appearance: CandidateAppearance,
): URL {
  return new URL(
    `./upstream/candidate-themes/${orientation}_candidate_window${appearance === "dark" ? "_dark" : ""}.html`,
    import.meta.url,
  );
}
