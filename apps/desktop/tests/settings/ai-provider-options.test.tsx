// @vitest-environment jsdom
import { expect, test } from "vitest";
import { render, screen } from "@testing-library/react";
import { AiProviderOptions } from "../../../../packages/ui/src/settings/ai-basic-settings-section";

test("renders AI provider option values and labels in order", () => {
  render(
    <select aria-label="AI 服务提供商">
      <AiProviderOptions
        options={[
          { id: "synthetic", title: "Synthetic" },
          { id: "other", title: "Other" },
        ]}
      />
    </select>,
  );

  const select = screen.getByRole("combobox", { name: "AI 服务提供商" }) as HTMLSelectElement;
  expect([...select.options].map((option) => [option.value, option.textContent])).toEqual([
    ["synthetic", "Synthetic"],
    ["other", "Other"],
  ]);
});
