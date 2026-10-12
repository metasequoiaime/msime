// @vitest-environment jsdom
import { expect, test } from "vitest";
import {
  animationVariables,
  parseAnimationVariable,
} from "../../../../packages/ui/src/skin/skin-animation-variables";

test.each([
  ["var(--name)", { name: "--name" }],
  ["var(--name,)", { name: "--name", fallback: "" }],
  ['var(--name, "a,b")', { name: "--name", fallback: '"a,b"' }],
  [
    "var(--name, var(--other, pulse 1s steps(2, end)))",
    { name: "--name", fallback: "var(--other, pulse 1s steps(2, end))" },
  ],
  [
    'var(--name, /* , ) */ var(--other, "a,b"))',
    { name: "--name", fallback: '/* , ) */ var(--other, "a,b")' },
  ],
])("parses whole variable references: %s", (value, expected) => {
  expect(parseAnimationVariable(value as string)).toEqual(expected);
});
test.each([
  "var(--a) pulse",
  "var(--a",
  "var(name)",
  "var(--)",
  "var(--a, 'bad)",
  "var(--a, /* bad)",
  "pulse",
])("does not misparse %s", (value) => {
  expect(parseAnimationVariable(value)).toBeNull();
});
function style(css: string) {
  const element = document.createElement("div");
  element.style.cssText = css;
  return element.style;
}
const literal = (value: string) => ({ value: "private-" + value, partial: false });
test("aliases preserve original definitions, priorities and dependency cycles", () => {
  const declaration = style("--a: var(--b) !important; --b: var(--a); --label: pulse;");
  const variables = animationVariables([declaration], "test-", literal);
  expect(variables.rewrite("var(--a, pulse)", "animation-name")).toBe(
    "var(--test-var-0, private-pulse)",
  );
  expect(variables.install()).toBe(false);
  expect(declaration.getPropertyValue("--a")).toBe("var(--b)");
  expect(declaration.getPropertyValue("--test-var-0")).toBe("var(--test-var-1)");
  expect(declaration.getPropertyValue("--test-var-1")).toBe("var(--test-var-0)");
  expect(declaration.getPropertyPriority("--test-var-0")).toBe("important");
  expect(declaration.getPropertyValue("--label")).toBe("pulse");
});
test("name and shorthand uses get distinct aliases without rewriting unrelated properties", () => {
  const declaration = style("--motion: pulse; color: red");
  const variables = animationVariables([declaration], "test-", literal);
  expect(variables.rewrite("var(--motion)", "animation-name")).toBe("var(--test-var-0)");
  expect(variables.rewrite("var(--motion)", "animation")).toBe("var(--test-var-1)");
  variables.install();
  expect(declaration.getPropertyValue("--motion")).toBe("pulse");
  expect(declaration.color).toBe("red");
});
test("caps recursive fallbacks and alias expansion", () => {
  const variables = animationVariables([], "test-", literal);
  variables.rewrite("var(--a,".repeat(40) + "pulse" + ")".repeat(40), "animation-name");
  expect(variables.install()).toBe(true);
  const bounded = animationVariables([], "bounded-", literal);
  for (let index = 0; index < 256; index++)
    bounded.rewrite("var(--v" + index + ")", "animation-name");
  expect(bounded.rewrite("var(--overflow)", "animation-name")).toBe("none");
  expect(bounded.install()).toBe(true);
});
test("generated aliases do not overwrite author properties or capture existing references", () => {
  const declaration = style("--test-var-0: foreign; --label: var(--test-var-1); --motion: pulse;");
  const variables = animationVariables([declaration], "test-", literal);
  expect(variables.rewrite("var(--motion)", "animation-name")).toBe("var(--test-var-2)");
  expect(variables.install()).toBe(false);
  expect(declaration.getPropertyValue("--test-var-0")).toBe("foreign");
  expect(declaration.getPropertyValue("--test-var-1")).toBe("");
  expect(declaration.getPropertyValue("--test-var-2")).toBe("private-pulse");
});
test("escaped references reserve the same logical alias name", () => {
  const declaration = style("--label: var(--\\74 est-var-0); --motion: pulse;");
  const variables = animationVariables([declaration], "test-", literal);
  expect(variables.rewrite("var(--motion)", "animation-name")).toBe("var(--test-var-1)");
  expect(variables.install()).toBe(false);
  expect(declaration.getPropertyValue("--test-var-0")).toBe("");
});
