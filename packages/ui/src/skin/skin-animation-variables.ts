export type AnimationMode = "animation" | "animation-name";
import { customPropertyNames, decodeCustomPropertyName } from "./css-custom-property";
import { scanCssValue } from "./css-value-scan";

// Recognize a whole var() value, preserving commas in strings/functions in its
// fallback. Partial token substitution is a separate compatibility step.
export function parseAnimationVariable(value: string): { name: string; fallback?: string } | null {
  value = value.trim();
  if (!/^var\(/i.test(value)) return null;
  let depth = 1,
    comma = -1;
  let result: { name: string; fallback?: string } | null = null;
  const valid = scanCssValue(value, 4, true, (char, index) => {
    if (char === "(") depth++;
    else if (char === ")") {
      if (--depth !== 0) return;
      if (index !== value.length - 1) return false;
      const name = decodeCustomPropertyName(value.slice(4, comma < 0 ? index : comma));
      if (name !== null)
        result = comma < 0 ? { name } : { name, fallback: value.slice(comma + 1, index).trim() };
      return false;
    } else if (char === "," && depth === 1 && comma < 0) comma = index;
  });
  return valid ? result : null;
}

// Duplicate only animation-referenced custom properties under private names.
// Original values stay intact for content, layout and other non-animation uses.
// Browser cascade/inheritance/cycle detection still resolves the alias graph.
export function animationVariables<Mode extends string = AnimationMode>(
  styles: CSSStyleDeclaration[],
  prefix: string,
  literal: (value: string, mode: Mode) => { value: string; partial: boolean },
) {
  const aliases = new Map<string, { source: string; mode: Mode; target: string }>();
  const reserved = new Set(styles.flatMap((style) => customPropertyNames(style.cssText)));
  let nextAlias = 0;
  let partial = false,
    expanded = 0;
  function rewrite(value: string, mode: Mode, depth = 0): string {
    if (depth > 32) {
      partial = true;
      return "none";
    }
    const variable = parseAnimationVariable(value);
    if (!variable) {
      const result = literal(value, mode);
      partial ||= result.partial;
      return result.value;
    }
    const key = mode + ":" + variable.name;
    let alias = aliases.get(key);
    if (!alias) {
      if (aliases.size >= 256) {
        partial = true;
        return "none";
      }
      let target: string;
      // Never reuse an author-provided variable or capture its references.
      do {
        target = "--" + prefix + "var-" + nextAlias++;
      } while (reserved.has(target));
      alias = { source: variable.name, mode, target };
      aliases.set(key, alias);
    }
    return (
      "var(" +
      alias.target +
      (variable.fallback === undefined
        ? ""
        : ", " + (variable.fallback === "" ? "" : rewrite(variable.fallback, mode, depth + 1))) +
      ")"
    );
  }
  function install() {
    // Map iteration includes dependencies discovered while processing values;
    // each (source, mode) is visited once, including cyclic graphs.
    for (const alias of aliases.values()) {
      for (const style of styles) {
        if (!Array.from(style).includes(alias.source)) continue;
        const original = style.getPropertyValue(alias.source);
        const value = /^(initial|inherit|unset|revert|revert-layer)$/i.test(original.trim())
          ? original
          : original.trim() === ""
            ? " "
            : rewrite(original, alias.mode);
        expanded += value.length;
        if (expanded > 16 * 1024 * 1024) {
          partial = true;
          style.setProperty(alias.target, "none");
        } else style.setProperty(alias.target, value, style.getPropertyPriority(alias.source));
      }
    }
    return partial;
  }
  return { rewrite, install };
}
