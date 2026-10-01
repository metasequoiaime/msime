// @vitest-environment jsdom
import { expect, test } from "vitest";
import {
  cssRuleChildren,
  isCssDeclarationRule,
  walkCssRules,
} from "../../../../packages/ui/src/skin/css-rules";

test("walks stylesheet rules and supported nested groups in source order", () => {
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(
    ".root{} @media (min-width: 1px){.media{} @supports (display: grid){.supported{}}} @keyframes fade{from{opacity:0}}",
  );
  const selectors: string[] = [];
  walkCssRules(sheet.cssRules, (rule) => {
    if (rule.type === CSSRule.STYLE_RULE) selectors.push((rule as CSSStyleRule).selectorText);
    else if (rule.type === CSSRule.KEYFRAME_RULE) selectors.push((rule as CSSKeyframeRule).keyText);
    else selectors.push(rule.constructor.name);
  });
  expect(selectors).toEqual([
    ".root",
    "CSSMediaRule",
    ".media",
    "CSSSupportsRule",
    ".supported",
    "CSSKeyframesRule",
    "0%",
  ]);
});

test("shares declaration and child-rule classification with sanitizers", () => {
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(".root{} @media (min-width: 1px){.nested{}} @font-face{font-family:demo}");
  const [root, media, font] = Array.from(sheet.cssRules);
  expect(isCssDeclarationRule(root)).toBe(true);
  expect(cssRuleChildren(root)).toHaveLength(0);
  expect(isCssDeclarationRule(media)).toBe(false);
  expect(cssRuleChildren(media)).toHaveLength(1);
  expect(isCssDeclarationRule(font)).toBe(false);
  expect(cssRuleChildren(font)).toBeUndefined();
});
