import { useEffect, useState } from "react";
import type { PackageCandidatePalette } from "../theme/global-theme";

/** The one package switch the `--cand-*` properties cannot carry: hiding the selection bar. Colours are never written as rules; they go through `customCandidateStyle`, layered as `resolve()` layers them. */
export function selectedBarCss(
  scope: string,
  palette: Pick<PackageCandidatePalette, "showSelectedBar"> | null,
): string[] {
  return palette?.showSelectedBar === false
    ? [`.${scope} .first::before{display:none !important}`]
    : [];
}

// Rules are generated from validated palette values, not arbitrary skin CSS.
// Constructed sheets preserve style-src 'self' without inline style elements.
export function installSkinPalette(
  rules: readonly string[],
  owner: Document = document,
): () => void {
  if (!("adoptedStyleSheets" in owner)) throw new Error("constructed stylesheets unavailable");
  const sheet = new CSSStyleSheet();
  for (const rule of rules) sheet.insertRule(rule, sheet.cssRules.length);
  owner.adoptedStyleSheets = [...owner.adoptedStyleSheets, sheet];
  return () => {
    owner.adoptedStyleSheets = owner.adoptedStyleSheets.filter((existing) => existing !== sheet);
  };
}

export function useSelectedBarPalette(scope: string, hideBar: boolean): boolean {
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!hideBar) {
      setFailed(false);
      return;
    }
    try {
      const remove = installSkinPalette(selectedBarCss(scope, { showSelectedBar: false }));
      setFailed(false);
      return remove;
    } catch {
      setFailed(true);
    }
  }, [hideBar, scope]);
  return failed;
}
