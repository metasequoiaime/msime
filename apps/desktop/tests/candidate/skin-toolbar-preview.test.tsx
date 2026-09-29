// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { SkinToolbarPreview } from "../../../../packages/ui/src/skin/skin-toolbar-preview";
import { utilityCss } from "../support/utility-css";

const css = utilityCss("skin-card-preview");
import type { FloatingToolbarPreferences } from "@msime/ui";

afterEach(cleanup);

test("draft components and size update the SVG toolbar including language visibility", () => {
  const preferences: FloatingToolbarPreferences = {
    enabled: true,
    english_mode: true,
    fullwidth: true,
    punctuation: true,
    character_set: true,
    emoji: true,
    handwriting: true,
    screen_keyboard: false,
    voice: true,
    settings: true,
    scale_percent: 100,
    font_size: 24,
  };
  const view = render(<SkinToolbarPreview preferences={preferences} />);
  const item = (key: string) =>
    view.container.querySelector<HTMLElement>(`[data-toolbar-item="${key}"]`)!;
  expect(item("language").style.display).toBe("flex");
  expect(item("screen_keyboard").style.display).toBe("none");
  for (const key of [
    "fullwidth",
    "punctuation",
    "character_set",
    "emoji",
    "screen_keyboard",
    "settings",
  ] as const) {
    view.rerender(<SkinToolbarPreview preferences={{ ...preferences, [key]: false }} />);
    expect(item(key).style.display).toBe("none");
    view.rerender(<SkinToolbarPreview preferences={{ ...preferences, [key]: true }} />);
    expect(item(key).style.display).toBe("flex");
  }
  view.rerender(<SkinToolbarPreview preferences={{ ...preferences, english_mode: false }} />);
  expect(item("language").style.display).toBe("none");
  view.rerender(<SkinToolbarPreview preferences={{ ...preferences, english_mode: true }} />);
  expect(item("language").style.display).toBe("flex");
  view.rerender(
    <SkinToolbarPreview
      preferences={{
        ...preferences,
        enabled: false,
        english_mode: false,
        scale_percent: 150,
        font_size: 28,
      }}
    />,
  );
  const host = view.container.querySelector<HTMLElement>(".ftb-preview-host")!;
  expect(host.style.getPropertyValue("--ftb-scale")).toBe("1.5");
  expect(host.style.getPropertyValue("--ftb-icon-size")).toBe("28px");
  expect(item("language").style.display).toBe("none");
  expect(host.querySelectorAll("svg").length).toBeGreaterThan(0);
});

test("toolbar preview contains upstream static icons without scripts, IDs or host actions", () => {
  const mounted = render(<SkinToolbarPreview />);
  const preview = mounted.container.querySelector(".ftb-preview-host")!;
  expect(preview.getAttribute("aria-hidden")).toBe("true");
  expect(preview.querySelectorAll(".status-bar")).toHaveLength(1);
  expect(preview.querySelector(".drag-handle")).not.toBeNull();
  expect(preview.querySelector(".divider")).not.toBeNull();
  expect(
    Array.from(preview.querySelectorAll("[data-toolbar-item]"), (node) =>
      node.getAttribute("data-toolbar-item"),
    ),
  ).toEqual([
    "language",
    "fullwidth",
    "punctuation",
    "character_set",
    "emoji",
    "screen_keyboard",
    "settings",
  ]);
  expect(preview.querySelectorAll("[data-toolbar-item=fullwidth] .icon")).toHaveLength(1);
  expect(preview.querySelectorAll("[data-toolbar-item=punctuation] .icon")).toHaveLength(1);
  expect(preview.querySelector('[title="英文"]')).toBeNull();
  expect(
    preview.querySelectorAll("script,iframe,object,embed,link,[id],button,a,input"),
  ).toHaveLength(0);
  for (const node of Array.from(preview.querySelectorAll("*"))) {
    for (const attribute of Array.from(node.attributes))
      expect(attribute.name).not.toMatch(/^(on|src$|href$|tabindex$)/i);
  }
});

// jsdom does not substitute custom properties, so the computed value is the declaration itself: a package's `--msime-toolbar-background`, falling back to the upstream colour.
test.each([
  ["dark", "var(--msime-toolbar-background, #1a1a1a)"],
  ["light", "var(--msime-toolbar-background, #ffffff)"],
])("%s toolbar uses the upstream palette", (appearance, surface) => {
  const mounted = render(
    <>
      <style>{css}</style>
      <div className="skin-card-preview" data-preview-theme={appearance}>
        <SkinToolbarPreview />
      </div>
    </>,
  );
  expect(getComputedStyle(mounted.container.querySelector(".status-bar")!).backgroundColor).toBe(
    surface,
  );
  expect(
    getComputedStyle(mounted.container.querySelector(".ftb-preview-host")!).pointerEvents,
  ).toBe("none");
  expect(getComputedStyle(mounted.container.querySelector(".icon svg")!).filter).toBe(
    appearance === "light" ? "invert(1)" : "none",
  );
});
