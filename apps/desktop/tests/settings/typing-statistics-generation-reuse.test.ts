import { expect, test } from "vitest";

test("统计页复用独立读取状态，状态继续复用公共挂载与代次生命周期", () => {
  const sources = import.meta.glob<string>(
    [
      "../../../../packages/ui/src/settings/typing-statistics.tsx",
      "../../../../packages/ui/src/settings/use-typing-statistics-state.ts",
    ],
    { eager: true, query: "?raw", import: "default" },
  );
  const page = sources["../../../../packages/ui/src/settings/typing-statistics.tsx"];
  const source = sources["../../../../packages/ui/src/settings/use-typing-statistics-state.ts"];

  expect(source).toBeDefined();
  expect(page).toMatch(/useTypingStatisticsState\(\s*client,\s*look,?\s*\)/);
  expect(page).not.toContain("requestStartedAtRef");
  expect(source).not.toMatch(/from ["'].*(?:typing-statistics["']|mobile-statistics|overview)/);
  expect(source).toContain("useMountedRef(");
  expect(source).toContain("useAsyncGeneration(");
  expect(source).not.toContain("let active = true");
  expect(source).not.toContain("const mounted = useRef(true)");
  expect(source).not.toContain("const clientGeneration = useRef(0)");
});
