// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { SettingsPage, themeEntry, type Snapshot } from "@msime/ui";

afterEach(cleanup);

const saved: Snapshot = {
  format_version: 1,
  revision: 7,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    candidate_corner_radius: 0,
    candidate_opacity_percent: 80,
  },
};

test("主题 cards draw the user's window radius and opacity over each theme, under the host's platform", async () => {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(saved),
        save: vi.fn(),
        host: testHost({ platform: "macos" }),
      }}
    />,
  );
  await screen.findByRole("region", { name: "主题列表" });
  const card = (id: string) =>
    document.querySelector<HTMLElement>(`article [data-skin-preview][data-global-theme="${id}"]`)!;
  for (const id of ["system", "light", "custom"]) {
    const style = card(id).style;
    expect(style.getPropertyValue("--msime-skin-radius")).toBe("0px");
    expect(style.getPropertyValue("--msime-candidate-user-radius")).toBe("0px");
    expect(style.getPropertyValue("--msime-candidate-opacity")).toBe("80%");
  }
  // 主题自己的配色照旧写在同一份内联样式上。
  expect(card("light").style.getPropertyValue("--cand-bg")).toBe(
    themeEntry("light").candidate!.surface,
  );
  // 样式表里 macOS 的默认值块挂在设置页根节点的 `data-platform="mac"` 下。
  expect(card("system").closest('[data-platform="mac"]')).not.toBeNull();
});
