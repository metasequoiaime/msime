// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import { AppearanceCandidatePreview } from "../../../../packages/ui/src/candidate/appearance-candidate-preview";
import { themeEntry, type Preferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const preferences: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
};

test("fixed themes keep their own appearance; system and custom resolve candidate override before global", () => {
  const view = render(<AppearanceCandidatePreview preferences={preferences} />);
  const preview = () => view.container.querySelector(".appearance-candidate-preview");
  expect(preview()?.getAttribute("data-preview-theme")).toBe("dark");
  const fixed = { shuishan: "dark", light: "light", paper: "light", night: "dark", ink: "dark" };
  for (const global_theme of ["system", "custom", ...Object.keys(fixed)] as const) {
    for (const candidate_theme of ["follow", "light", "dark"] as const) {
      view.rerender(
        <AppearanceCandidatePreview
          preferences={{
            ...preferences,
            global_theme: global_theme as Preferences["global_theme"],
            theme: "light",
            settings_theme: "dark",
            candidate_theme,
          }}
        />,
      );
      expect(preview()?.getAttribute("data-global-theme")).toBe(global_theme);
      expect(preview()?.getAttribute("data-preview-theme")).toBe(
        fixed[global_theme as keyof typeof fixed] ??
          (candidate_theme === "dark" ? "dark" : "light"),
      );
    }
  }
});

test("a built-in theme draws its catalog palette; system leaves the platform tokens alone", () => {
  const view = render(
    <AppearanceCandidatePreview preferences={{ ...preferences, global_theme: "paper" }} />,
  );
  const style = () =>
    (view.container.querySelector(".appearance-candidate-preview") as HTMLElement).style;
  const paper = themeEntry("paper").candidate!;
  expect(style().getPropertyValue("--cand-bg")).toBe(paper.surface);
  expect(style().getPropertyValue("--cand-accent")).toBe(paper.accent);
  view.rerender(
    <AppearanceCandidatePreview preferences={{ ...preferences, global_theme: "system" }} />,
  );
  expect(style().getPropertyValue("--cand-bg")).toBe("");
  expect(style().getPropertyValue("--cand-accent")).toBe("");
});

test("a custom theme over a built-in base keeps that base's mode and palette under the pickers", () => {
  const view = render(
    <AppearanceCandidatePreview
      preferences={{
        ...preferences,
        theme: "light",
        global_theme: "custom",
        custom_theme: { base: "night", candidate_colors: { text: "#ff0000", accent: "#112233" } },
      }}
    />,
  );
  const preview = view.container.querySelector(".appearance-candidate-preview") as HTMLElement;
  const night = themeEntry("night").candidate!;
  // The base fixes the mode whatever the host draws in, as resolve() does.
  expect(preview.getAttribute("data-preview-theme")).toBe("dark");
  expect(preview.style.getPropertyValue("--cand-bg")).toBe(night.surface);
  // Picker values come back normalized to upper case, as resolve() returns them.
  expect(preview.style.getPropertyValue("--cand-text")).toBe("#FF0000");
  // A text picker carries the numbers; the selected row and the hover fill follow the picked accent and text.
  expect(preview.style.getPropertyValue("--cand-num")).toBe("#FF00009D");
  expect(preview.style.getPropertyValue("--cand-selected")).toBe("#11223324");
  expect(preview.style.getPropertyValue("--cand-hover")).toBe("#FF00000F");
  view.rerender(
    <AppearanceCandidatePreview
      preferences={{
        ...preferences,
        theme: "light",
        global_theme: "custom",
        custom_theme: { candidate_colors: { accent: "#112233" } },
      }}
    />,
  );
  // Over the system base only the picked slots are drawn.
  expect(preview.getAttribute("data-preview-theme")).toBe("light");
  expect(preview.style.getPropertyValue("--cand-accent")).toBe("#112233");
  expect(preview.style.getPropertyValue("--cand-bg")).toBe("");
  expect(preview.style.getPropertyValue("--cand-selected")).toBe("");
});

test("system changes update following previews and listeners are disposed on override/unmount", () => {
  let listener: (() => void) | undefined;
  const media = {
    matches: true,
    addEventListener: vi.fn((_event, callback) => {
      listener = callback;
    }),
    removeEventListener: vi.fn(() => {
      listener = undefined;
    }),
  };
  vi.stubGlobal(
    "matchMedia",
    vi.fn(() => media),
  );
  const view = render(
    <AppearanceCandidatePreview preferences={{ ...preferences, theme: "system" }} />,
  );
  const theme = () =>
    view.container
      .querySelector(".appearance-candidate-preview")
      ?.getAttribute("data-preview-theme");
  expect(theme()).toBe("light");
  act(() => {
    media.matches = false;
    listener!();
  });
  expect(theme()).toBe("dark");
  view.rerender(
    <AppearanceCandidatePreview
      preferences={{ ...preferences, theme: "system", candidate_theme: "light" }}
    />,
  );
  expect(theme()).toBe("light");
  expect(media.removeEventListener).toHaveBeenCalledTimes(1);
  view.rerender(<AppearanceCandidatePreview preferences={{ ...preferences, theme: "system" }} />);
  expect(theme()).toBe("dark");
  view.unmount();
  expect(media.removeEventListener).toHaveBeenCalledTimes(2);
});

test("missing system API retains the upstream dark fallback", () => {
  vi.stubGlobal("matchMedia", undefined);
  const view = render(
    <AppearanceCandidatePreview preferences={{ ...preferences, theme: "system" }} />,
  );
  expect(
    view.container
      .querySelector(".appearance-candidate-preview")
      ?.getAttribute("data-preview-theme"),
  ).toBe("dark");
});
