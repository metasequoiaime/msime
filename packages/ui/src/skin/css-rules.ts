type CssRuleContainer = CSSStyleSheet | CSSGroupingRule | CSSStyleRule;

export function isCssNestedDeclaration(rule: CSSRule): boolean {
  return rule.constructor.name === "CSSNestedDeclarations";
}

export function isCssDeclarationRule(rule: CSSRule): boolean {
  return (
    rule.type === CSSRule.STYLE_RULE ||
    rule.type === CSSRule.KEYFRAME_RULE ||
    isCssNestedDeclaration(rule)
  );
}

export function cssRuleChildren(rule: CSSRule): CSSRuleList | undefined {
  if (isCssNestedDeclaration(rule)) return undefined;
  if (
    rule.type !== CSSRule.STYLE_RULE &&
    rule.type !== CSSRule.MEDIA_RULE &&
    rule.type !== CSSRule.SUPPORTS_RULE &&
    rule.type !== CSSRule.KEYFRAMES_RULE
  )
    return undefined;
  return (rule as CssRuleContainer).cssRules;
}

export function walkCssRules(rules: CSSRuleList, visit: (rule: CSSRule) => void): void {
  for (const rule of Array.from(rules)) {
    visit(rule);
    const nested = cssRuleChildren(rule);
    if (nested) walkCssRules(nested, visit);
  }
}

export async function walkCssRulesAsync(
  rules: CSSRuleList,
  visit: (rule: CSSRule) => void | Promise<void>,
): Promise<void> {
  for (const rule of Array.from(rules)) {
    await visit(rule);
    const nested = cssRuleChildren(rule);
    if (nested) await walkCssRulesAsync(nested, visit);
  }
}
