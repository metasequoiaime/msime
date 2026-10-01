import { decodeCssUrl } from "./css-image-value";
import { fontFamilyKey } from "./font-family";
import { animationVariables } from "./skin-animation-variables";
import { preserveAnimationShorthands, preserveFontShorthands } from "./animation-shorthand-source";
import { installConditionalFonts, type ConditionalFont } from "./conditional-fonts";
import { walkCssRules } from "./css-rules";

export function splitCssFontList(value: string): string[] {
  const parts: string[] = [];
  let start = 0,
    depth = 0,
    quote = "";
  for (let index = 0; index < value.length; index++) {
    const char = value[index];
    if (char === "\\") {
      index++;
      continue;
    }
    if (quote) {
      if (char === quote) quote = "";
      continue;
    }
    if (char === '"' || char === "'") quote = char;
    else if (char === "(") depth++;
    else if (char === ")") depth--;
    else if (char === "," && depth === 0) {
      parts.push(value.slice(start, index).trim());
      start = index + 1;
    }
  }
  parts.push(value.slice(start).trim());
  return parts;
}

export function fontPackagePath(source: string): string | null {
  const match =
    /^url\(\s*(?:"((?:[^"\\]|\\[\s\S])*)"|'((?:[^'\\]|\\[\s\S])*)'|([^\s)'"]*))\s*\)(?:\s+format\(\s*["']?(?:woff2?|truetype|opentype)["']?\s*\))?$/i.exec(
      source,
    );
  if (!match) return null;
  const decoded = decodeCssUrl(match[1] ?? match[2] ?? match[3], match[3] === undefined);
  const relative = decoded?.replace(/^\.\//, "");
  return relative &&
    relative.length <= 256 &&
    relative
      .split("/")
      .every((part) => part !== "." && part !== ".." && /^[a-zA-Z0-9._-]+$/.test(part))
    ? relative
    : null;
}

const genericFamilies =
  /^(serif|sans-serif|monospace|cursive|fantasy|system-ui|ui-serif|ui-sans-serif|ui-monospace|ui-rounded|math|fangsong|inherit|initial|unset|revert|revert-layer)$/i;
let generation = 0;

export async function prepareToolbarFonts(
  css: string,
  resolve: (relative: string) => Promise<ArrayBuffer>,
) {
  const fontSource = preserveFontShorthands(css);
  const preserved = preserveAnimationShorthands(fontSource.css);
  preserved.partial ||= fontSource.partial;
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(preserved.css);
  const definitions: { style: CSSStyleDeclaration; media: MediaQueryList[]; supported: boolean }[] =
    [];
  const queries = new Map<string, MediaQueryList>();
  function collect(
    container: CSSStyleSheet | CSSGroupingRule | CSSStyleRule,
    media: MediaQueryList[],
    supported: boolean,
  ) {
    for (const rule of Array.from(container.cssRules)) {
      if (rule.type === CSSRule.FONT_FACE_RULE)
        definitions.push({ style: (rule as CSSFontFaceRule).style, media, supported });
      else if (rule.type === CSSRule.MEDIA_RULE) {
        const condition = (rule as CSSMediaRule).conditionText;
        let query = queries.get(condition);
        if (!query) {
          query = matchMedia(condition);
          queries.set(condition, query);
        }
        collect(rule as CSSMediaRule, [...media, query], supported);
      } else if (rule.type === CSSRule.SUPPORTS_RULE) {
        collect(
          rule as CSSSupportsRule,
          media,
          supported && CSS.supports((rule as CSSSupportsRule).conditionText),
        );
      } else if (rule.type === CSSRule.STYLE_RULE && (rule as CSSStyleRule).cssRules?.length) {
        collect(rule as CSSStyleRule, media, supported);
      }
    }
    for (let index = container.cssRules.length - 1; index >= 0; index--) {
      if (container.cssRules[index].type === CSSRule.FONT_FACE_RULE) container.deleteRule(index);
    }
  }
  collect(sheet, [], true);
  if (!definitions.length)
    return { css: preserved.css, partial: preserved.partial, install: () => () => {} };
  let partial = preserved.partial,
    total = 0,
    count = 0;
  const prefix = "msime-skin-font-" + ++generation + "-";
  const families = new Map<string, string>();
  const loaded: ConditionalFont[] = [];
  const cache = new Map<string, Promise<ArrayBuffer>>();
  const read = (relative: string) => {
    let result = cache.get(relative);
    if (!result) {
      if (cache.size >= 16) return Promise.reject(new Error("font count exceeded"));
      result = resolve(relative).then((bytes) => {
        total += bytes.byteLength;
        if (!bytes.byteLength || bytes.byteLength > 8 * 1024 * 1024 || total > 16 * 1024 * 1024)
          throw new Error("font budget exceeded");
        return bytes;
      });
      cache.set(relative, result);
    }
    return result;
  };
  for (const { style, media, supported } of definitions) {
    const key = fontFamilyKey(style.getPropertyValue("font-family"));
    if (!key) {
      partial = true;
      continue;
    }
    if (!families.has(key)) families.set(key, prefix + families.size);
    if (!supported) continue;
    if (++count > 8 || typeof FontFace === "undefined") {
      partial = true;
      continue;
    }
    const descriptors: FontFaceDescriptors = {};
    for (const [cssName, descriptor] of [
      ["font-style", "style"],
      ["font-weight", "weight"],
      ["font-stretch", "stretch"],
      ["unicode-range", "unicodeRange"],
      ["font-feature-settings", "featureSettings"],
      ["font-variation-settings", "variationSettings"],
      ["font-display", "display"],
      ["ascent-override", "ascentOverride"],
      ["descent-override", "descentOverride"],
      ["line-gap-override", "lineGapOverride"],
    ] as const) {
      const value = style.getPropertyValue(cssName);
      if (value) Object.assign(descriptors, { [descriptor]: value });
    }
    let found = false;
    for (const source of splitCssFontList(style.getPropertyValue("src"))) {
      const path = fontPackagePath(source);
      if (!path) {
        partial = true;
        continue;
      }
      try {
        const face = new FontFace(families.get(key)!, await read(path), descriptors);
        await face.load();
        loaded.push({ face, media });
        found = true;
        break;
      } catch {
        /* Try the next package source without exposing decoder errors. */
      }
    }
    if (!found) partial = true;
  }
  const styles: CSSStyleDeclaration[] = [];
  walkCssRules(sheet.cssRules, (rule) => {
    if (rule.type === CSSRule.FONT_FACE_RULE) partial = true;
    if (
      rule.type === CSSRule.STYLE_RULE ||
      rule.type === CSSRule.KEYFRAME_RULE ||
      rule.constructor.name === "CSSNestedDeclarations"
    )
      styles.push((rule as CSSStyleRule).style);
  });
  const familyParser = new CSSStyleSheet();
  familyParser.insertRule(".family-parser {}", 0);
  const familyStyle = (familyParser.cssRules[0] as CSSStyleRule).style;
  const variables = animationVariables<"font-family">(styles, prefix, (value) => {
    if (/\bvar\s*\(/i.test(value)) return { value: "sans-serif", partial: true };
    familyStyle.cssText = "";
    familyStyle.setProperty("font-family", value);
    const canonical = familyStyle.getPropertyValue("font-family");
    if (!canonical) return { value: "sans-serif", partial: true };
    return {
      value: splitCssFontList(canonical)
        .map((family) =>
          genericFamilies.test(family)
            ? family
            : (families.get(fontFamilyKey(family) ?? "") ?? family),
        )
        .join(", "),
      partial: false,
    };
  });
  for (const style of styles) {
    if (!Array.from(style).includes("font-family")) continue;
    const value = style.getPropertyValue("font-family");
    if (!value) {
      style.setProperty("font-family", "sans-serif");
      partial = true;
      continue;
    }
    style.setProperty(
      "font-family",
      variables.rewrite(value, "font-family"),
      style.getPropertyPriority("font-family"),
    );
  }
  partial = variables.install() || partial;
  return {
    css: Array.from(sheet.cssRules)
      .map((rule) => rule.cssText)
      .join("\n"),
    partial,
    install: () => installConditionalFonts(loaded),
  };
}
