import { expect, test } from "vitest";
import { aiProviderOption } from "../../../../packages/ui/src/settings/ai-provider-options";

test("looks up an AI provider catalog entry by id", () => {
  expect(aiProviderOption("openai")).toMatchObject({
    id: "openai",
    title: "OpenAI",
  });
  expect(aiProviderOption("missing-provider")).toBeUndefined();
});
