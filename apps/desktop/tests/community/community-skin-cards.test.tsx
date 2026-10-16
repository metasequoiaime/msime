// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import type { FormEvent } from "react";
import {
  CommunitySkinCard,
  CommunityHarmonySkinCard,
  type CommunitySkin,
  type CommunitySkinPillState,
} from "@msime/ui";
import * as style from "../../../../packages/ui/src/community/community-style";

afterEach(cleanup);

function skin(overrides: Partial<CommunitySkin> = {}): CommunitySkin {
  return {
    id: "synthetic-skin",
    name: "合成皮肤",
    description: "合成说明",
    author: "合成作者",
    design: {
      background: 0x123456,
      keyBackground: 0xffffff,
      keyForeground: 0x17251d,
      accent: 0x185c47,
      actionBackground: 0x185c47,
      cornerRadius: 8,
      borderWidth: 0,
      shadow: 0,
      pattern: 0,
      monospaced: false,
    },
    category: "nature",
    downloads: 12345,
    rating_count: 2,
    rating_average: 4.25,
    owned: false,
    my_rating: 0,
    ...overrides,
  };
}

test("skin card draws its compact custom preview and opens without submitting a parent form", () => {
  const open = vi.fn();
  const submit = vi.fn((event: FormEvent) => event.preventDefault());
  render(
    <form onSubmit={submit}>
      <CommunitySkinCard skin={skin()} theme="dark" open={open} />
    </form>,
  );
  const card = screen.getByRole("button", { name: "查看皮肤 合成皮肤" });
  expect(within(card).getByText("自然 · 合成作者")).toBeTruthy();
  expect(within(card).getByText("↓ 12,345")).toBeTruthy();
  expect(within(card).getByText("☆ 4.3 分")).toBeTruthy();
  const preview = card.querySelector("svg")!;
  expect(preview.getAttribute("data-preview-theme")).toBe("dark");
  expect(preview.getAttribute("data-preview-skin")).toBe("custom");
  expect(preview.getAttribute("class")).toContain("compact");
  expect(preview.getAttribute("aria-hidden")).toBe("true");
  expect(preview.querySelector("rect")?.getAttribute("fill")).toBe("#123456");
  card.click();
  expect(open).toHaveBeenCalledOnce();
  expect(submit).not.toHaveBeenCalled();
});

test("skin card marks only its owner's removed work and accepts uncategorized unrated skins", () => {
  const view = render(
    <CommunitySkinCard
      skin={skin({ owned: true, moderation: "removed" })}
      theme="light"
      open={vi.fn()}
    />,
  );
  expect(screen.getByText("自然 · 我的作品 · 已下架")).toBeTruthy();
  view.rerender(
    <CommunitySkinCard
      skin={skin({ category: undefined, moderation: "removed", rating_count: 0 })}
      theme="light"
      open={vi.fn()}
    />,
  );
  expect(screen.getByText("合成作者")).toBeTruthy();
  expect(screen.getByText("☆ 暂无评分")).toBeTruthy();
  expect(screen.queryByText(/我的作品|已下架|自然/)).toBeNull();
});

test("Harmony skin card opens from text or preview and retains a decorative phone thumbnail", () => {
  const open = vi.fn();
  const act = vi.fn();
  const view = render(
    <CommunityHarmonySkinCard skin={skin()} theme="light" pill="get" open={open} act={act} />,
  );
  expect(screen.getByText("自然 · 合成作者")).toBeTruthy();
  expect(screen.getByText("12,345 次使用")).toBeTruthy();
  expect(screen.queryByText(/☆|合成说明/)).toBeNull();
  const preview = view.container.querySelector("svg")!;
  expect(preview.getAttribute("viewBox")).toBe("0 0 390 292");
  expect(preview.getAttribute("data-preview-theme")).toBe("light");
  expect(preview.getAttribute("data-preview-skin")).toBe("custom");
  expect(preview.getAttribute("aria-hidden")).toBe("true");
  expect(preview.querySelector("[data-keyboard-toolbar]")).toBeTruthy();
  expect(preview.querySelector("rect")?.getAttribute("fill")).toBe("#123456");
  fireEvent.click(screen.getByRole("button", { name: "查看皮肤 合成皮肤" }));
  fireEvent.click(preview);
  expect(open).toHaveBeenCalledTimes(2);
  expect(act).not.toHaveBeenCalled();
});

test.each<[CommunitySkinPillState, string, boolean]>([
  ["get", "获取", false],
  ["getting", "获取中", true],
  ["use", "使用", false],
  ["using", "使用", true],
  ["in-use", "使用中", true],
])(
  "Harmony %s pill preserves its label, disabled state and action boundary",
  (pill, label, disabled) => {
    const open = vi.fn();
    const act = vi.fn();
    const submit = vi.fn((event: FormEvent) => event.preventDefault());
    render(
      <form onSubmit={submit}>
        <CommunityHarmonySkinCard skin={skin()} theme="dark" pill={pill} open={open} act={act} />
      </form>,
    );
    const button = screen.getByRole("button", {
      name: `${label}皮肤 合成皮肤`,
    }) as HTMLButtonElement;
    expect(button.textContent).toBe(label);
    expect(button.disabled).toBe(disabled);
    expect(button.className).toContain(
      pill === "in-use" ? style.harmonyPillDone : style.harmonyPillTonal,
    );
    button.click();
    expect(act).toHaveBeenCalledTimes(disabled ? 0 : 1);
    expect(open).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "查看皮肤 合成皮肤" }));
    expect(open).toHaveBeenCalledOnce();
  },
);

test("Harmony metadata marks removed owned skins and omits a missing category and author", () => {
  const view = render(
    <CommunityHarmonySkinCard
      skin={skin({ owned: true, moderation: "removed" })}
      theme="light"
      pill="get"
      open={vi.fn()}
      act={vi.fn()}
    />,
  );
  expect(screen.getByText("自然 · 我的作品 · 已下架")).toBeTruthy();
  view.rerender(
    <CommunityHarmonySkinCard
      skin={skin({ author: "", category: undefined, moderation: "removed" })}
      theme="light"
      pill="get"
      open={vi.fn()}
      act={vi.fn()}
    />,
  );
  expect(screen.queryByText(/我的作品|已下架|自然|合成作者/)).toBeNull();
  expect(screen.getByText("12,345 次使用")).toBeTruthy();
});

test("Harmony skin card reflects updated skin, theme, pill and callbacks", () => {
  const stale = vi.fn();
  const open = vi.fn();
  const act = vi.fn();
  const view = render(
    <CommunityHarmonySkinCard
      skin={skin()}
      theme="light"
      pill="getting"
      open={stale}
      act={stale}
    />,
  );
  view.rerender(
    <CommunityHarmonySkinCard
      skin={skin({ name: "另一合成皮肤" })}
      theme="dark"
      pill="use"
      open={open}
      act={act}
    />,
  );
  expect(view.container.querySelector("svg")?.getAttribute("data-preview-theme")).toBe("dark");
  fireEvent.click(screen.getByRole("button", { name: "使用皮肤 另一合成皮肤" }));
  fireEvent.click(screen.getByRole("button", { name: "查看皮肤 另一合成皮肤" }));
  expect(open).toHaveBeenCalledOnce();
  expect(act).toHaveBeenCalledOnce();
  expect(stale).not.toHaveBeenCalled();
});
