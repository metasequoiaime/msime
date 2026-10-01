import { animationVariables, type AnimationMode } from "./skin-animation-variables";
import { walkCssRules } from "./css-rules";
// Names are decoded by the browser using the same grammar as @keyframes.
// A sticky token scan keeps quoted/escaped commas inside their name.
export function rewriteAnimationNames(
  value: string,
  names: ReadonlyMap<string, string>,
): { value: string; partial: boolean } {
  const token =
    /"(?:[^"\\]|\\[\s\S])*"|'(?:[^'\\]|\\[\s\S])*'|(?:\\(?:[0-9a-f]{1,6}(?:\r\n|[ \t\r\n\f])?|[\s\S])|[^\s,'"()\\])+/giy;
  const output: string[] = [];
  const parser = new CSSStyleSheet();
  let offset = 0,
    partial = false;
  while (offset < value.length) {
    while (/\s/.test(value[offset] ?? "")) offset++;
    token.lastIndex = offset;
    const match = token.exec(value);
    if (!match) return { value: "none", partial: true };
    offset = token.lastIndex;
    if (/^(none|initial|unset)$/i.test(match[0])) output.push("none");
    else {
      parser.replaceSync("@keyframes " + match[0] + " {}");
      const rule = parser.cssRules[0] as CSSKeyframesRule | undefined;
      const replacement =
        parser.cssRules.length === 1 && rule?.type === CSSRule.KEYFRAMES_RULE
          ? names.get(rule.name)
          : undefined;
      output.push(replacement ?? "none");
      if (!replacement) partial = true;
    }
    while (/\s/.test(value[offset] ?? "")) offset++;
    if (offset === value.length) break;
    if (value[offset++] !== "," || offset === value.length) return { value: "none", partial: true };
  }
  return { value: output.join(", ") || "none", partial: partial || !output.length };
}

let generation = 0;
export function isolateToolbarAnimations(sheet: CSSStyleSheet): boolean {
  const prefix = "msime-skin-animation-" + ++generation + "-";
  const names = new Map<string, string>();
  walkCssRules(sheet.cssRules, (rule) => {
    if (rule.type !== CSSRule.KEYFRAMES_RULE) return;
    const frames = rule as CSSKeyframesRule;
    if (!names.has(frames.name)) names.set(frames.name, prefix + names.size);
    frames.name = names.get(frames.name)!;
  });
  const styles: CSSStyleDeclaration[] = [];
  walkCssRules(sheet.cssRules, (rule) => {
    if (
      rule.type === CSSRule.STYLE_RULE ||
      rule.type === CSSRule.KEYFRAME_RULE ||
      rule.constructor.name === "CSSNestedDeclarations"
    )
      styles.push((rule as CSSStyleRule).style);
  });
  const parser = new CSSStyleSheet();
  parser.insertRule(".animation-parser {}", 0);
  const parsed = (parser.cssRules[0] as CSSStyleRule).style;
  const variables = animationVariables(styles, prefix, (value: string, mode: AnimationMode) => {
    if (mode === "animation-name") return rewriteAnimationNames(value, names);
    parsed.cssText = "";
    parsed.setProperty("animation", value);
    const rewritten = rewriteAnimationNames(parsed.getPropertyValue("animation-name"), names);
    parsed.setProperty("animation-name", rewritten.value);
    return { value: parsed.getPropertyValue("animation") || "none", partial: rewritten.partial };
  });
  for (const style of styles) {
    if (!Array.from(style).includes("animation-name")) continue;
    const name = style.getPropertyValue("animation-name");
    const mode = name ? "animation-name" : "animation";
    // Use the shorthand only for pending variable substitution. Static names
    // still change just the longhand, preserving independent timing overrides.
    style.setProperty(
      mode,
      variables.rewrite(name || style.getPropertyValue("animation"), mode),
      style.getPropertyPriority(mode),
    );
  }
  return variables.install();
}
