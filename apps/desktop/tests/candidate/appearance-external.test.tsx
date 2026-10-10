// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { AppearanceCandidatePreview } from "../../../../packages/ui/src/candidate/appearance-candidate-preview";
import type { Preferences, SkinCatalog } from "@msime/ui";

afterEach(cleanup);
beforeEach(() =>
  Object.defineProperty(document, "adoptedStyleSheets", {
    configurable: true,
    writable: true,
    value: [],
  }),
);
const preferences: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 4,
  learning: true,
  chinese_punctuation: true,
  global_theme: "custom",
  custom_theme: { candidate_skin: "sample" },
};
const catalog: SkinCatalog = {
  directory: "/synthetic/skins",
  issues: [],
  packages: [
    {
      id: "sample",
      name: "Sample",
      version: "1",
      base: "system",
      author: null,
      description: null,
      layouts: ["horizontal", "vertical"],
      themes: ["dark"],
      minWidthDip: 100,
      decorationTopDip: 20,
      decorationWidthDip: 100,
      toolbarStylesheet: null,
      preview: "sample.svg",
      candidate: { dark: { surface: "#123456" }, light: {} },
    },
  ],
};
// The preview draws package colours as `--cand-*` properties on the preview element; only a hidden selection bar needs a stylesheet.
function drawn(container: HTMLElement, property: string): string {
  const preview = container.querySelector<HTMLElement>(".appearance-candidate-preview")!;
  return preview.style.getPropertyValue(property);
}
const image = {
  contentType: "image/svg+xml",
  bytes: [...new TextEncoder().encode('<svg xmlns="http://www.w3.org/2000/svg"/>')],
};

test("theme changes replace external palette without rescanning or reloading images", async () => {
  const scan = vi.fn().mockResolvedValue({
    ...catalog,
    packages: [
      {
        ...catalog.packages[0],
        themes: ["dark", "light"],
        candidate: { dark: { surface: "#123456" }, light: { surface: "#abcdef" } },
      },
    ],
  });
  const readImage = vi.fn().mockResolvedValue(image);
  const view = render(
    <AppearanceCandidatePreview preferences={preferences} scan={scan} readImage={readImage} />,
  );
  await waitFor(() => expect(view.container.querySelector("img")).not.toBeNull());
  for (const candidate_theme of ["light", "dark", "light"] as const) {
    view.rerender(
      <AppearanceCandidatePreview
        preferences={{ ...preferences, candidate_theme }}
        scan={scan}
        readImage={readImage}
      />,
    );
    expect(
      view.container
        .querySelector(".appearance-candidate-preview")
        ?.getAttribute("data-preview-theme"),
    ).toBe(candidate_theme);
    expect(drawn(view.container, "--cand-bg")).toBe(
      candidate_theme === "light" ? "#ABCDEF" : "#123456",
    );
    expect(document.adoptedStyleSheets).toHaveLength(0);
  }
  expect(scan).toHaveBeenCalledTimes(1);
  expect(readImage).toHaveBeenCalledTimes(1);
});

test("each mode draws only its own package palette, and the selection bar follows the drawn mode", async () => {
  const scan = vi.fn().mockResolvedValue({
    ...catalog,
    packages: [
      {
        ...catalog.packages[0],
        themes: ["dark", "light"],
        candidate: {
          dark: { surface: "#123456", border: "#112233", showSelectedBar: false },
          light: { surface: "#abcdef" },
        },
      },
    ],
  });
  const view = render(
    <AppearanceCandidatePreview
      preferences={{ ...preferences, candidate_theme: "light" }}
      scan={scan}
    />,
  );
  await waitFor(() => expect(drawn(view.container, "--cand-bg")).toBe("#ABCDEF"));
  // The light palette is not layered over the dark one: resolve() reads one mode.
  expect(drawn(view.container, "--cand-border")).toBe("");
  expect(document.adoptedStyleSheets).toHaveLength(0);
  view.rerender(<AppearanceCandidatePreview preferences={preferences} scan={scan} />);
  expect(drawn(view.container, "--cand-bg")).toBe("#123456");
  expect(drawn(view.container, "--cand-border")).toBe("#112233");
  expect(document.adoptedStyleSheets).toHaveLength(1);
  expect(document.adoptedStyleSheets[0].cssRules[0].cssText).toContain("display: none");
  view.unmount();
  expect(document.adoptedStyleSheets).toHaveLength(0);
});

test("the pickers draw over the package and derived slots follow the package, as resolve() layers them", async () => {
  const packaged = (light: Record<string, string>) =>
    vi.fn().mockResolvedValue({
      ...catalog,
      packages: [
        {
          ...catalog.packages[0],
          base: "paper",
          themes: ["light"],
          candidate: { dark: {}, light },
        },
      ],
    });
  // paper 底的皮肤是浅色皮肤，只在浅色模式下画。
  const light = { ...preferences, candidate_theme: "light" as const };
  const view = render(
    <AppearanceCandidatePreview
      preferences={{
        ...light,
        custom_theme: { candidate_skin: "sample", candidate_colors: { text: "#ff0000" } },
      }}
      scan={packaged({ text: "#112233" })}
    />,
  );
  await waitFor(() => expect(drawn(view.container, "--cand-text")).toBe("#FF0000"));
  expect(drawn(view.container, "--cand-num")).toBe("#FF00009D");
  view.unmount();
  const accent = render(
    <AppearanceCandidatePreview preferences={light} scan={packaged({ accent: "#AA0000" })} />,
  );
  await waitFor(() => expect(drawn(accent.container, "--cand-selected")).toBe("#AA000024"));
  expect(drawn(accent.container, "--accent-strong")).toBe("#AA0000");
});

// 预览按当前明暗取槽位：浅色模式画 `candidate_skin`，深色模式画 `candidate_skin_dark`；只设了浅色皮肤时深色模式不画它，画底和取色器。
test("the preview draws the skin of the previewed mode's slot", async () => {
  const scan = vi.fn().mockResolvedValue({
    ...catalog,
    packages: [
      {
        ...catalog.packages[0],
        id: "paperish",
        base: "paper",
        themes: ["light"],
        candidate: { dark: {}, light: { surface: "#abcdef" } },
      },
      {
        ...catalog.packages[0],
        id: "nightish",
        base: "night",
        themes: ["dark"],
        candidate: { dark: { surface: "#123456" }, light: {} },
      },
    ],
  });
  const slots = {
    ...preferences,
    custom_theme: {
      base: "night" as const,
      candidate_skin: "paperish",
      candidate_skin_dark: "nightish",
    },
  };
  const view = render(
    <AppearanceCandidatePreview preferences={{ ...slots, candidate_theme: "light" }} scan={scan} />,
  );
  await waitFor(() => expect(drawn(view.container, "--cand-bg")).toBe("#ABCDEF"));
  view.rerender(
    <AppearanceCandidatePreview preferences={{ ...slots, candidate_theme: "dark" }} scan={scan} />,
  );
  await waitFor(() => expect(drawn(view.container, "--cand-bg")).toBe("#123456"));
  view.unmount();

  const lightOnly = render(
    <AppearanceCandidatePreview
      preferences={{
        ...preferences,
        candidate_theme: "dark",
        custom_theme: { base: "paper", candidate_skin: "paperish" },
      }}
      scan={scan}
    />,
  );
  await screen.findByText("所选皮肤是浅色皮肤，深色模式下不使用它。");
  // 设过皮肤、这个模式却没有可画的包：paper 不属于深色，底按 `system` 画，不画浅色皮肤的配色。
  const plain = lightOnly.container.querySelector<HTMLElement>(".appearance-candidate-preview")!;
  expect(plain.getAttribute("data-preview-theme")).toBe("dark");
  expect(plain.style.getPropertyValue("--cand-bg")).not.toBe("#ABCDEF");
  expect(lightOnly.container.querySelector(".skin-decoration-image")).toBeNull();
});

test("a host with a theme call previews its own resolve() answer", async () => {
  const resolveTheme = vi.fn().mockResolvedValue({
    id: "custom",
    source: "custom",
    appearance: null,
    candidate: {
      surface: "#010203",
      border: null,
      text: null,
      number: null,
      secondary: null,
      accent: null,
      selected: null,
      selected_text: null,
      selected_number: null,
      hover: null,
      show_selected_bar: false,
    },
    keyboard: null,
    candidate_skin: "sample",
  });
  const view = render(
    <AppearanceCandidatePreview
      preferences={preferences}
      scan={async () => catalog}
      resolveTheme={resolveTheme}
    />,
  );
  await waitFor(() => expect(drawn(view.container, "--cand-bg")).toBe("#010203"));
  expect(resolveTheme).toHaveBeenCalledWith({
    global_theme: "custom",
    custom_theme: preferences.custom_theme,
    dark: true,
    layout: "vertical",
  });
  expect(document.adoptedStyleSheets).toHaveLength(1);
});

test("unsupported light mode removes dark preview and recovers on switching back", async () => {
  const scan = vi.fn().mockResolvedValue(catalog);
  const view = render(<AppearanceCandidatePreview preferences={preferences} scan={scan} />);
  await waitFor(() => expect(view.container.querySelector(".candidate")).not.toBeNull());
  view.rerender(
    <AppearanceCandidatePreview
      preferences={{ ...preferences, candidate_theme: "light" }}
      scan={scan}
    />,
  );
  expect(screen.getByText(/不支持当前布局或浅色模式/)).not.toBeNull();
  expect(view.container.querySelector(".candidate")).toBeNull();
  expect(document.adoptedStyleSheets).toHaveLength(0);
  view.rerender(<AppearanceCandidatePreview preferences={preferences} scan={scan} />);
  expect(view.container.querySelector(".candidate")).not.toBeNull();
  expect(scan).toHaveBeenCalledTimes(1);
});

test("external appearance loads palette/image, updates draft, refreshes and cleans up", async () => {
  const scan = vi.fn().mockResolvedValue(catalog),
    readImage = vi.fn().mockResolvedValue(image);
  const view = render(
    <AppearanceCandidatePreview preferences={preferences} scan={scan} readImage={readImage} />,
  );
  await waitFor(() =>
    expect(view.container.querySelector("img.skin-decoration-image")).not.toBeNull(),
  );
  expect(scan).toHaveBeenCalledTimes(1);
  expect(readImage).toHaveBeenCalledExactlyOnceWith("sample", "sample.svg");
  expect(view.container.querySelectorAll(".cand")).toHaveLength(4);
  expect(drawn(view.container, "--cand-bg")).toBe("#123456");
  view.rerender(
    <AppearanceCandidatePreview
      preferences={{ ...preferences, candidate_layout: "horizontal", candidate_page_size: 9 }}
      scan={scan}
      readImage={readImage}
    />,
  );
  expect(view.container.querySelectorAll(".wnd-h .cand")).toHaveLength(9);
  expect(scan).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "刷新预览" }));
  await waitFor(() => expect(readImage).toHaveBeenCalledTimes(2));
  expect(drawn(view.container, "--cand-bg")).toBe("#123456");
  expect(document.adoptedStyleSheets).toHaveLength(0);
});

test("late catalog from a replaced host cannot overwrite the current preview", async () => {
  let resolve!: (value: SkinCatalog) => void;
  const scan = () =>
    new Promise<SkinCatalog>((done) => {
      resolve = done;
    });
  const view = render(<AppearanceCandidatePreview preferences={preferences} scan={scan} />);
  const replacement = vi.fn().mockResolvedValue({ ...catalog, packages: [] });
  view.rerender(<AppearanceCandidatePreview preferences={preferences} scan={replacement} />);
  await screen.findByText(/未找到所选皮肤/);
  await act(async () => resolve(catalog));
  expect(view.container.querySelector(".candidate")).toBeNull();
  expect(document.adoptedStyleSheets).toHaveLength(0);
});

test("hidden appearance does not scan and switching to a builtin discards pending results", async () => {
  let resolve!: (value: SkinCatalog) => void;
  const scan = vi.fn(
    () =>
      new Promise<SkinCatalog>((done) => {
        resolve = done;
      }),
  );
  const readImage = vi.fn();
  const view = render(
    <AppearanceCandidatePreview
      preferences={preferences}
      scan={scan}
      readImage={readImage}
      active={false}
    />,
  );
  expect(scan).not.toHaveBeenCalled();
  view.rerender(
    <AppearanceCandidatePreview preferences={preferences} scan={scan} readImage={readImage} />,
  );
  expect(scan).toHaveBeenCalledTimes(1);
  view.rerender(
    <AppearanceCandidatePreview
      preferences={{ ...preferences, global_theme: "night" }}
      scan={scan}
      readImage={readImage}
    />,
  );
  await act(async () => resolve(catalog));
  expect(view.container.querySelector('[data-global-theme="night"]')).not.toBeNull();
  expect(readImage).not.toHaveBeenCalled();
  expect(document.adoptedStyleSheets).toHaveLength(0);
});

test("scan failure can retry; incompatible layout never shows a misleading candidate", async () => {
  const scan = vi
    .fn()
    .mockRejectedValueOnce(Error("synthetic failure"))
    .mockResolvedValue({
      ...catalog,
      packages: [{ ...catalog.packages[0], layouts: ["horizontal"] }],
    });
  const view = render(<AppearanceCandidatePreview preferences={preferences} scan={scan} />);
  await screen.findByText(/读取所选皮肤失败/);
  fireEvent.click(screen.getByRole("button", { name: "刷新预览" }));
  await screen.findByText(/所选皮肤不支持/);
  expect(view.container.querySelector(".candidate")).toBeNull();
  expect(document.adoptedStyleSheets).toHaveLength(0);
});

test("image failures retain palette and report the limitation", async () => {
  const view = render(
    <AppearanceCandidatePreview
      preferences={preferences}
      scan={async () => catalog}
      readImage={async () => {
        throw Error("synthetic failure");
      }}
    />,
  );
  await screen.findByText(/皮肤图片加载失败/);
  expect(view.container.querySelectorAll(".cand")).toHaveLength(4);
  expect(drawn(view.container, "--cand-bg")).toBe("#123456");
});
