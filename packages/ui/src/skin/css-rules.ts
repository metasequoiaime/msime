type CssRuleContainer = CSSStyleSheet | CSSGroupingRule | CSSStyleRule;

function isNestedDeclaration(rule: CSSRule): boolean {
  return rule.constructor.name === "CSSNestedDeclarations";
}

function childRules(rule: CSSRule): CSSRuleList | undefined {
  if (isNestedDeclaration(rule)) return undefined;
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
    const nested = childRules(rule);
    if (nested) walkCssRules(nested, visit);
  }
}

export async function walkCssRulesAsync(
  rules: CSSRuleList,
  visit: (rule: CSSRule) => void | Promise<void>,
): Promise<void> {
  for (const rule of Array.from(rules)) {
    await visit(rule);
    const nested = childRules(rule);
    if (nested) await walkCssRulesAsync(nested, visit);
  }
}
