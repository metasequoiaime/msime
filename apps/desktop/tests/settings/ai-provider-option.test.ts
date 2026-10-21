import { expect, test } from "vitest";
import {
  aiProviderOption,
  aiProviderOptionsFor,
  aiProviderUsesToken,
} from "../../../../packages/ui/src/settings/ai-provider-options";

test("looks up an AI provider catalog entry by id", () => {
  expect(aiProviderOption("openai")).toMatchObject({
    id: "openai",
    title: "OpenAI",
  });
  expect(aiProviderOption("missing-provider")).toBeUndefined();
});

test("Ollama is offered on the desktop only and goes without a token", () => {
  expect(aiProviderOption("ollama")).toMatchObject({
    endpoint: "http://localhost:11434/v1/chat/completions",
    model: "",
  });
  expect(aiProviderUsesToken("ollama")).toBe(false);
  expect(aiProviderUsesToken("custom")).toBe(true);
  const ids = (desktop: boolean, selected: string) =>
    aiProviderOptionsFor(desktop, selected).map((option) => option.id);
  expect(ids(true, "openai")).toContain("ollama");
  expect(ids(false, "openai")).not.toContain("ollama");
  // 已经选着 Ollama 的手机设置仍然显示得出当前值。
  expect(ids(false, "ollama")).toContain("ollama");
  expect(ids(true, "openai").at(-1)).toBe("custom");
});
