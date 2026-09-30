// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within, waitFor } from "@testing-library/react";
import { ExternalSkins, selectedBarCss } from "../../../../packages/ui/src/skin/external-skins";
import { SettingsPage, type SkinCatalog, type Snapshot } from "@msime/ui";
import { utilityCss } from "../support/utility-css";

const geometryCss = utilityCss("external-skin-decorated");
import { skinImageUrl, type SkinImage } from "../../../../packages/ui/src/skin/skin-image";
import desktopConfig from "../../src-tauri/tauri.conf.json";
import * as fontPreparation from "../../../../packages/ui/src/skin/toolbar-fonts";

afterEach(cleanup);
// jsdom parses CSS rules but does not implement adopted stylesheet rendering.
// Real CSP/computed-style coverage lives in scripts/test-skin-palette-csp.py.
beforeEach(() =>
  Object.defineProperty(document, "adoptedStyleSheets", {
    configurable: true,
    writable: true,
    value: [],
  }),
);
// Package colours are `--cand-*` properties on the card preview; only a hidden selection bar needs a stylesheet.
function drawn(card: HTMLElement, property: string): string {
  return card.querySelector<HTMLElement>("[data-skin-preview]")!.style.getPropertyValue(property);
}
function previewCss(card: HTMLElement): string {
  const scope = Array.from(card.querySelector("[data-skin-preview]")!.classList).find((value) =>
    value.startsWith("external-preview-"),
  )!;
  return document.adoptedStyleSheets
    .flatMap((sheet) => Array.from(sheet.cssRules).map((rule) => rule.cssText))
    .filter((rule) => rule.includes(`.${scope} `))
    .join("")
    .replace(/\s+/g, "");
}
const catalog: SkinCatalog = {
  directory: "/synthetic/state/skins",
  issues: [{ folder: "Bad", reason: "invalid manifest" }],
  packages: [
    {
      id: "sample",
      name: "Sample skin",
      version: "1",
      base: "system",
      author: "Example",
      description: "Sample description",
      layouts: ["horizontal"],
      themes: ["dark", "light"],
      minWidthDip: 0,
      decorationTopDip: 0,
      decorationWidthDip: 0,
      toolbarStylesheet: null,
      preview: null,
      candidate: { dark: { surface: "#123456" }, light: { surface: "#abcdef" } },
    },
  ],
};
const initial: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    candidate_layout: "horizontal",
    learning: true,
    chinese_punctuation: true,
  },
};
const props = { selected: "", layout: "horizontal", onSelect: vi.fn() };

test("external skin selected-bar flag emits a scoped hide rule", () => {
  expect(selectedBarCss("scope", { showSelectedBar: false })).toEqual([
    ".scope .first::before{display:none !important}",
  ]);
  expect(selectedBarCss("scope", { showSelectedBar: true })).toEqual([]);
  expect(selectedBarCss("scope", null)).toEqual([]);
});
// The list scans once as it mounts; a manual refresh is clicked once that scan has settled and the button is back.
async function refresh() {
  fireEvent.click(await screen.findByRole("button", { name: "刷新皮肤" }));
}

test("settings forwards the declared toolbar reader using only package id", async () => {
  const readSkinToolbarCss = vi.fn().mockResolvedValue(null);
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        readSkinToolbarCss,
        scanSkinCatalog: async () => ({
          ...catalog,
          packages: [{ ...catalog.packages[0], toolbarStylesheet: "toolbar.css" }],
        }),
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  await waitFor(() => expect(readSkinToolbarCss).toHaveBeenCalledExactlyOnceWith("sample"));
});
test("settings forwards the font reader with package id and relative name", async () => {
  const readSkinFont = vi.fn().mockResolvedValue({ contentType: "font/woff2", bytes: [0, 1] });
  const prepare = vi
    .spyOn(fontPreparation, "prepareToolbarFonts")
    .mockImplementation(async (_css, resolve) => {
      await resolve("fonts/test.woff2");
      return { css: "", partial: false, install: () => () => {} };
    });
  try {
    render(
      <SettingsPage
        client={{
          load: async () => initial,
          save: vi.fn(),
          readSkinFont,
          readSkinToolbarCss: async () => ".sample {}",
          scanSkinCatalog: async () => ({
            ...catalog,
            packages: [{ ...catalog.packages[0], toolbarStylesheet: "toolbar.css" }],
          }),
        }}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "主题" }));
    await waitFor(() =>
      expect(readSkinFont).toHaveBeenCalledExactlyOnceWith("sample", "fonts/test.woff2"),
    );
  } finally {
    prepare.mockRestore();
  }
});

test("the selection bar sheet follows the drawn mode and disappears when cards unmount", async () => {
  const mounted = render(
    <ExternalSkins
      {...props}
      scan={async () => ({
        ...catalog,
        packages: [
          {
            ...catalog.packages[0],
            candidate: {
              dark: { surface: "#123456", showSelectedBar: false },
              light: { surface: "#abcdef" },
            },
          },
        ],
      })}
    />,
  );
  const card = await screen.findByRole("article");
  expect(card.querySelector("style")).toBeNull();
  await waitFor(() => expect(document.adoptedStyleSheets).toHaveLength(1));
  expect(drawn(card, "--cand-bg")).toBe("#123456");
  fireEvent.click(within(card).getByRole("button", { name: "预览浅色" }));
  expect(document.adoptedStyleSheets).toHaveLength(0);
  expect(drawn(card, "--cand-bg")).toBe("#ABCDEF");
  fireEvent.click(within(card).getByRole("button", { name: "预览深色" }));
  expect(document.adoptedStyleSheets).toHaveLength(1);
  mounted.unmount();
  expect(document.adoptedStyleSheets).toHaveLength(0);
});

test("missing adopted stylesheets reports fallback without injecting inline styles", async () => {
  Reflect.deleteProperty(document, "adoptedStyleSheets");
  render(
    <ExternalSkins
      {...props}
      scan={async () => ({
        ...catalog,
        packages: [
          {
            ...catalog.packages[0],
            candidate: { dark: { showSelectedBar: false }, light: {} },
          },
        ],
      })}
    />,
  );
  await screen.findByText("当前浏览器无法隐藏皮肤的选中条，其余配色照常预览。");
  expect(screen.getByRole("article").querySelector("style")).toBeNull();
});

const imageCatalog: SkinCatalog = {
  ...catalog,
  packages: [
    {
      ...catalog.packages[0],
      preview: "images/top.png",
      decorationTopDip: 32,
      decorationWidthDip: 150,
    },
  ],
};
const imageData: SkinImage = { contentType: "image/png", bytes: [0, 1, 255] };

test("image data URLs preserve bytes and reject non-image or invalid payloads", () => {
  expect(skinImageUrl(imageData)).toBe("data:image/png;base64,AAH/");
  for (const image of [
    { contentType: "text/html", bytes: [] },
    { contentType: "image/png;bad", bytes: [] },
    { ...imageData, bytes: [-1] },
    { ...imageData, bytes: [256] },
    { ...imageData, bytes: [1.5] },
    { ...imageData, bytes: [NaN] },
    { ...imageData, bytes: new Array(8 * 1024 * 1024 + 1) },
  ]) {
    expect(() => skinImageUrl(image)).toThrow("invalid image");
  }
});

test("one host image read feeds both preview layouts and refresh reloads unchanged manifests", async () => {
  const readImage = vi.fn().mockResolvedValue(imageData);
  const mounted = render(
    <ExternalSkins {...props} scan={async () => imageCatalog} readImage={readImage} />,
  );
  expect(readImage).not.toHaveBeenCalled();
  const card = await screen.findByRole("article");
  await waitFor(() => expect(card.querySelectorAll("img.skin-decoration-image")).toHaveLength(2));
  expect(readImage).toHaveBeenCalledExactlyOnceWith("sample", "images/top.png");
  expect(card.querySelector("img.skin-decoration-image")?.getAttribute("src")).toBe(
    skinImageUrl(imageData),
  );
  fireEvent.click(within(card).getByRole("button", { name: "预览浅色" }));
  expect(readImage).toHaveBeenCalledTimes(1);
  await refresh();
  await waitFor(() => expect(readImage).toHaveBeenCalledTimes(2));
  mounted.unmount();
});

test("settings forwards image reader and existing CSP permits image data without broader sources", async () => {
  const readSkinImage = vi.fn().mockResolvedValue(imageData);
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save: vi.fn(),
        scanSkinCatalog: async () => imageCatalog,
        readSkinImage,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  await waitFor(() =>
    expect(readSkinImage).toHaveBeenCalledExactlyOnceWith("sample", "images/top.png"),
  );
  const imgDirective = desktopConfig.app.security.csp
    .split(";")
    .find((value) => value.trim().startsWith("img-src"));
  expect(imgDirective?.trim()).toBe("img-src 'self' data:");
});

test("image read and decode failures retain base preview and can be retried", async () => {
  const readImage = vi
    .fn()
    .mockRejectedValueOnce(new Error("private diagnostic"))
    .mockResolvedValue(imageData);
  render(<ExternalSkins {...props} scan={async () => imageCatalog} readImage={readImage} />);
  await screen.findByText("皮肤图片加载失败，保留基础预览。可刷新皮肤重试。");
  expect(screen.queryByText("private diagnostic")).toBeNull();
  await refresh();
  const card = screen.getByRole("article");
  await waitFor(() => expect(card.querySelector("img.skin-decoration-image")).not.toBeNull());
  fireEvent.error(card.querySelector("img.skin-decoration-image")!);
  expect(card.querySelector("img.skin-decoration-image")).toBeNull();
  expect(card.querySelectorAll(".candidate .container")).toHaveLength(2);
  await refresh();
  await waitFor(() => expect(card.querySelectorAll("img.skin-decoration-image")).toHaveLength(2));
});

test("old image response cannot replace current resource after catalog refresh", async () => {
  let finish!: (image: SkinImage) => void;
  const readImage = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise<SkinImage>((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValue({ ...imageData, bytes: [2] });
  render(<ExternalSkins {...props} scan={async () => imageCatalog} readImage={readImage} />);
  await waitFor(() => expect(readImage).toHaveBeenCalledTimes(1));
  await refresh();
  const card = screen.getByRole("article");
  await waitFor(() =>
    expect(card.querySelector("img.skin-decoration-image")?.getAttribute("src")).toContain("Ag=="),
  );
  await act(async () => finish(imageData));
  expect(card.querySelector("img.skin-decoration-image")?.getAttribute("src")).toContain("Ag==");
});

test("images are not requested without decoration and optional hosts remain usable", async () => {
  const readImage = vi.fn();
  const scan = async () => ({
    ...imageCatalog,
    packages: [{ ...imageCatalog.packages[0], decorationTopDip: 0, decorationWidthDip: 0 }],
  });
  const mounted = render(<ExternalSkins {...props} scan={scan} readImage={readImage} />);
  await screen.findByRole("article");
  expect(readImage).not.toHaveBeenCalled();
  mounted.rerender(<ExternalSkins {...props} scan={async () => imageCatalog} />);
  await screen.findByText("当前宿主不支持皮肤图片预览。");
});

test("decorated previews preserve upstream geometry in both layouts without decorating toolbar", async () => {
  render(
    <ExternalSkins
      {...props}
      scan={async () => ({
        ...catalog,
        packages: [
          {
            ...catalog.packages[0],
            minWidthDip: 280.5,
            decorationTopDip: 32.5,
            decorationWidthDip: 150,
          },
        ],
      })}
    />,
  );
  const card = await screen.findByRole("article");
  expect(card.classList.contains("external-skin-decorated")).toBe(true);
  const preview = card.querySelector<HTMLElement>("[data-skin-preview]")!;
  expect(preview.style.getPropertyValue("--msime-skin-min-width")).toBe("280.5px");
  expect(preview.style.getPropertyValue("--msime-skin-decoration-top")).toBe("32.5px");
  expect(preview.style.getPropertyValue("--msime-skin-decoration-width")).toBe("150px");
  for (const layout of ["horizontal", "vertical"]) {
    expect(
      card.querySelector(`[data-preview-layout="${layout}"] > .containerParent > .container`),
    ).not.toBeNull();
  }
  expect(card.querySelectorAll(".containerParent")).toHaveLength(2);
  expect(card.querySelector("[data-skin-stage]:last-child .containerParent")).toBeNull();
});

test.each([
  [0, 0],
  [12, 0],
  [0, 12],
  [-1, 12],
  [501, 12],
  [12, 1001],
  [Infinity, 12],
  [12, NaN],
])(
  "invalid or absent decoration %s/%s retains plain candidate markup",
  async (decorationTopDip, decorationWidthDip) => {
    render(
      <ExternalSkins
        {...props}
        scan={async () => ({
          ...catalog,
          packages: [
            { ...catalog.packages[0], minWidthDip: Infinity, decorationTopDip, decorationWidthDip },
          ],
        })}
      />,
    );
    const card = await screen.findByRole("article");
    expect(card.classList.contains("external-skin-decorated")).toBe(false);
    expect(card.querySelector(".containerParent")).toBeNull();
    const preview = card.querySelector<HTMLElement>("[data-skin-preview]")!;
    expect(preview.style.getPropertyValue("--msime-skin-min-width")).toBe("0px");
    expect(preview.style.getPropertyValue("--msime-skin-decoration-top")).toBe("0px");
    expect(preview.style.getPropertyValue("--msime-skin-decoration-width")).toBe("0px");
  },
);

test("refresh removes stale geometry and independent cards do not inherit it", async () => {
  const scan = vi
    .fn()
    .mockResolvedValueOnce({
      ...catalog,
      packages: [
        {
          ...catalog.packages[0],
          decorationTopDip: 500,
          decorationWidthDip: 1000,
          minWidthDip: 1000,
        },
        { ...catalog.packages[0], id: "plain", name: "Plain" },
      ],
    })
    .mockResolvedValue(catalog);
  render(<ExternalSkins {...props} scan={scan} />);
  const card = await screen.findByRole("article", { name: "Sample skin" });
  expect(card.querySelectorAll(".containerParent")).toHaveLength(2);
  expect(
    screen.getByRole("article", { name: "Plain" }).querySelector(".containerParent"),
  ).toBeNull();
  await refresh();
  await act(async () => {});
  expect(card.querySelector(".containerParent")).toBeNull();
  expect(card.classList.contains("external-skin-decorated")).toBe(false);
});

test("geometry stylesheet overhangs the mascot into the card, above it, as every host draws it", () => {
  const style = document.createElement("style");
  style.textContent = geometryCss;
  document.head.append(style);
  try {
    const rules = Array.from(style.sheet!.cssRules) as CSSStyleRule[];
    const parent = rules.find(
      (rule) => rule.selectorText === ".external-skin-decorated .candidate .containerParent",
    )!;
    // The stage is the band taller than the card, and shrinks to the card so the edges it aligns to are the card's.
    expect(parent.style.getPropertyValue("padding-top")).toBe(
      "var(--msime-skin-decoration-top, 0px)",
    );
    expect(parent.style.getPropertyValue("width")).toBe("fit-content");
    expect(
      parent.style.getPropertyValue("--msime-skin-decoration-bottom").replace(/\s+/g, ""),
    ).toBe("calc(var(--msime-skin-decoration-top,0px)+var(--msime-candidate-pad-y,0px))");
    const mascot = rules.find((rule) =>
      rule.selectorText?.split(/,\s*/).includes(".external-skin-decorated .skin-decoration-image"),
    )!;
    expect(mascot.selectorText).toContain("::before");
    // Bottom pad_y below the card's top edge, at most band + pad_y tall, never squashed.
    expect(mascot.style.getPropertyValue("bottom")).toBe(
      "calc(100% - var(--msime-skin-decoration-bottom))",
    );
    expect(mascot.style.getPropertyValue("width")).toBe("var(--msime-skin-decoration-width, 0px)");
    expect(mascot.style.getPropertyValue("height")).toBe("auto");
    expect(mascot.style.getPropertyValue("max-height")).toBe("var(--msime-skin-decoration-bottom)");
    expect(mascot.style.getPropertyValue("object-fit")).toBe("contain");
    expect(mascot.style.getPropertyValue("object-position")).toBe("right bottom");
    // Right by default: card_right - pad_x - width, never left of the card.
    expect(mascot.style.getPropertyValue("left").replace(/\s+/g, "")).toBe(
      "max(0px,calc(100%-var(--msime-candidate-pad-x,0px)-var(--msime-skin-decoration-width,0px)))",
    );
    expect(mascot.style.getPropertyValue("pointer-events")).toBe("none");
    const container = rules.find(
      (rule) => rule.selectorText === ".external-skin-decorated .candidate .container",
    )!;
    // Drawn over the card.
    expect(Number(mascot.style.getPropertyValue("z-index"))).toBeGreaterThan(
      Number(container.style.getPropertyValue("z-index")),
    );
    expect(container.style.getPropertyValue("min-width")).toBe(
      "max(7em, var(--msime-skin-min-width, 0px))",
    );
    expect(geometryCss).not.toContain("url(");
  } finally {
    style.remove();
  }
});

test("a card draws one declared mode over its base, never the other mode beneath it", async () => {
  render(
    <ExternalSkins
      {...props}
      scan={async () => ({
        ...catalog,
        packages: [
          {
            ...catalog.packages[0],
            base: "paper",
            themes: ["light"],
            candidate: {
              dark: { surface: "#123456", text: "#112233" },
              light: { accent: "#aa0000" },
            },
          },
        ],
      })}
    />,
  );
  const card = await screen.findByRole("article");
  // Paper fixes the light mode; its surface and text show through where the light palette is silent.
  expect(drawn(card, "--cand-bg")).toBe("#F7F5F0");
  expect(drawn(card, "--cand-text")).toBe("#1A1E1B");
  expect(drawn(card, "--accent-strong")).toBe("#AA0000");
  expect(drawn(card, "--cand-selected")).toBe("#AA000024");
  expect(previewCss(card)).toBe("");
});

test("open directory is explicit, path-free and independent of scanning and selection", async () => {
  const openDirectory = vi.fn().mockResolvedValue(undefined);
  const scan = vi.fn().mockResolvedValue(catalog);
  const onSelect = vi.fn();
  render(
    <ExternalSkins {...props} openDirectory={openDirectory} scan={scan} onSelect={onSelect} />,
  );
  expect(openDirectory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
  await screen.findByRole("button", { name: "打开目录" });
  expect(openDirectory).toHaveBeenCalledWith();
  // Only the scan the list makes as it mounts; opening the folder does not rescan.
  expect(scan).toHaveBeenCalledTimes(1);
  expect(onSelect).not.toHaveBeenCalled();
});

test("settings forwards the open directory capability", async () => {
  const openSkinDirectory = vi.fn().mockResolvedValue(undefined);
  const save = vi.fn();
  render(<SettingsPage client={{ load: async () => initial, save, openSkinDirectory }} />);
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
  await screen.findByRole("button", { name: "打开目录" });
  expect(openSkinDirectory).toHaveBeenCalledWith();
  expect(save).not.toHaveBeenCalled();
});

test("directory opener is disabled when unavailable and retries sanitized failures", async () => {
  const mounted = render(<ExternalSkins {...props} />);
  expect((screen.getByRole("button", { name: "打开目录" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  const openDirectory = vi
    .fn()
    .mockImplementationOnce(() => {
      throw new Error("private diagnostic");
    })
    .mockResolvedValue(undefined);
  mounted.rerender(<ExternalSkins {...props} openDirectory={openDirectory} />);
  fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
  expect((await screen.findByRole("alert")).textContent).toBe("无法打开皮肤目录，请重试。");
  expect(screen.queryByText("private diagnostic")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
  await screen.findByRole("button", { name: "打开目录" });
  expect(screen.queryByRole("alert")).toBeNull();
  expect(openDirectory).toHaveBeenCalledTimes(2);
});

test("opening deduplicates requests and ignores late failures after host replacement", async () => {
  let reject!: (error: Error) => void;
  const openDirectory = vi.fn(
    () =>
      new Promise<void>((_resolve, fail) => {
        reject = fail;
      }),
  );
  const mounted = render(<ExternalSkins {...props} openDirectory={openDirectory} />);
  fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
  fireEvent.click(screen.getByRole("button", { name: "正在打开…" }));
  expect(openDirectory).toHaveBeenCalledTimes(1);
  mounted.rerender(<ExternalSkins {...props} openDirectory={async () => {}} />);
  await act(async () => reject(new Error("synthetic")));
  expect(screen.queryByRole("alert")).toBeNull();
  expect((screen.getByRole("button", { name: "打开目录" }) as HTMLButtonElement).disabled).toBe(
    false,
  );
});

test("catalog is scanned as the list mounts and displays host directory, metadata and diagnostics", async () => {
  const scan = vi.fn().mockResolvedValue(catalog);
  render(<ExternalSkins {...props} scan={scan} />);
  expect(screen.getByRole("status").textContent).toContain("正在读取皮肤目录");
  const card = await screen.findByRole("article", { name: "Sample skin" });
  expect(scan).toHaveBeenCalledExactlyOnceWith();
  expect(screen.getByText(catalog.directory)).toBeTruthy();
  expect(within(card).getByText("sample · v1 · Example")).toBeTruthy();
  expect(screen.getByText("已忽略 1 个无效皮肤目录")).toBeTruthy();
  expect(screen.getByText("Bad：invalid manifest")).toBeTruthy();
  expect(card.querySelectorAll("[data-skin-stage]")).toHaveLength(3);
});

test("external selection enters the revisioned draft; preview toggles never save or select", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  render(
    <SettingsPage
      client={{ load: async () => initial, save, scanSkinCatalog: async () => catalog }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  const card = await screen.findByRole("article", { name: "Sample skin" });
  fireEvent.click(within(card).getByRole("button", { name: "预览浅色" }));
  expect(within(card).getByRole("switch").getAttribute("aria-checked")).toBe("false");
  expect(drawn(card, "--cand-bg")).toBe("#ABCDEF");
  expect(save).not.toHaveBeenCalled();
  fireEvent.click(within(card).getByRole("switch"));
  fireEvent.click(within(card).getByRole("switch"));
  expect(within(card).getByRole("switch").getAttribute("aria-checked")).toBe("true");
  expect(save).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
  expect(save).toHaveBeenCalledWith(
    3,
    expect.objectContaining({
      global_theme: "custom",
      custom_theme: expect.objectContaining({ base: "system", candidate_skin: "sample" }),
    }),
  );
});

test("light-only skin compatibility follows actual theme, not card override", async () => {
  const scan = vi
    .fn()
    .mockResolvedValue({ ...catalog, packages: [{ ...catalog.packages[0], themes: ["light"] }] });
  const onSelect = vi.fn();
  const view = render(
    <ExternalSkins {...props} onSelect={onSelect} scan={scan} activeTheme="dark" />,
  );
  const toggle = await screen.findByRole("switch");
  expect((toggle as HTMLButtonElement).disabled).toBe(true);
  view.rerender(<ExternalSkins {...props} onSelect={onSelect} scan={scan} activeTheme="light" />);
  expect((toggle as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "预览深色" }));
  expect((toggle as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(toggle);
  expect(onSelect).toHaveBeenCalledExactlyOnceWith("sample", "system");
  view.rerender(<ExternalSkins {...props} onSelect={onSelect} scan={scan} activeTheme="dark" />);
  expect((toggle as HTMLButtonElement).disabled).toBe(true);
  expect(
    view.container.querySelector("[data-skin-preview]")?.getAttribute("data-preview-theme"),
  ).toBe("light");
  expect(scan).toHaveBeenCalledTimes(1);
});

test("a package over a built-in base is drawn in that base's mode whatever the host mode", async () => {
  const night = { ...catalog.packages[0], base: "night" as const, themes: ["light"] };
  const onSelect = vi.fn();
  const view = render(
    <ExternalSkins
      {...props}
      onSelect={onSelect}
      scan={async () => ({ ...catalog, packages: [night] })}
      activeTheme="light"
    />,
  );
  const card = await screen.findByRole("article", { name: "Sample skin" });
  // Night fixes the dark mode, so there is no mode to switch the preview to.
  expect(card.querySelector("[data-skin-preview]")?.getAttribute("data-preview-theme")).toBe(
    "dark",
  );
  expect(within(card).queryByRole("button", { name: /预览/ })).toBeNull();
  // The package declares only light, so over its dark base it adds no colours and night is drawn alone.
  expect(previewCss(card)).toBe("");
  // The host mode does not rule the package out: resolve() draws it the same in either mode.
  const toggle = within(card).getByRole("switch") as HTMLButtonElement;
  expect(toggle.disabled).toBe(false);
  fireEvent.click(toggle);
  expect(onSelect).toHaveBeenCalledExactlyOnceWith("sample", "night");
  view.rerender(
    <ExternalSkins
      {...props}
      onSelect={onSelect}
      scan={async () => ({ ...catalog, packages: [night] })}
      activeTheme="dark"
    />,
  );
  expect(toggle.disabled).toBe(false);
});

test("settings synchronize all cards and reset local overrides on candidate theme changes", async () => {
  const save = vi.fn(),
    scan = vi.fn().mockResolvedValue(catalog);
  render(<SettingsPage client={{ load: async () => initial, save, scanSkinCatalog: scan }} />);
  const mode = (name: string) =>
    fireEvent.click(
      within(screen.getByRole("radiogroup", { name: "颜色模式" })).getByRole("radio", { name }),
    );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  mode("浅色");
  await screen.findByRole("article", { name: "Sample skin" });
  expect(screen.getAllByRole("article")).toHaveLength(8);
  // The built-in themes are fixed palettes with no preview switch; the system card, the custom card over the system base and the external package follow the mode.
  const cards = ["跟随系统", "自定义", "Sample skin"].map((name) =>
    screen.getByRole("article", { name }),
  );
  for (const card of cards) {
    expect(card.querySelector("[data-skin-preview]")?.getAttribute("data-preview-theme")).toBe(
      "light",
    );
    fireEvent.click(within(card).getByRole("button", { name: "预览深色" }));
    expect(card.querySelector("[data-skin-preview]")?.getAttribute("data-preview-theme")).toBe(
      "dark",
    );
  }
  mode("深色");
  mode("浅色");
  for (const card of cards)
    expect(card.querySelector("[data-skin-preview]")?.getAttribute("data-preview-theme")).toBe(
      "light",
    );
  expect(scan).toHaveBeenCalledTimes(1);
  expect(save).not.toHaveBeenCalled();
});

test("manifest compatibility follows actual layout and dark host theme, not preview override", async () => {
  const onSelect = vi.fn();
  const mounted = render(
    <ExternalSkins {...props} onSelect={onSelect} scan={async () => catalog} layout="vertical" />,
  );
  const toggle = await screen.findByRole("switch", { name: "Sample skin" });
  expect((toggle as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(toggle);
  fireEvent.click(screen.getByRole("button", { name: "预览浅色" }));
  expect((toggle as HTMLButtonElement).disabled).toBe(true);
  expect(onSelect).not.toHaveBeenCalled();
  mounted.unmount();
  render(
    <ExternalSkins
      {...props}
      scan={async () => ({ ...catalog, packages: [{ ...catalog.packages[0], themes: ["light"] }] })}
    />,
  );
  expect(((await screen.findByRole("switch")) as HTMLButtonElement).disabled).toBe(true);
});

test("settings without a layout use the same vertical default for skin compatibility", async () => {
  render(
    <SettingsPage
      client={{
        load: async () => ({
          ...initial,
          preferences: { ...initial.preferences, candidate_layout: undefined },
        }),
        save: vi.fn(),
        scanSkinCatalog: async () => catalog,
      }}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "主题" }));
  expect(
    ((await screen.findByRole("switch", { name: "Sample skin" })) as HTMLButtonElement).disabled,
  ).toBe(true);
});

test("errors retain last catalog, hide raw exception and permit retry to empty results", async () => {
  const scan = vi
    .fn()
    .mockResolvedValueOnce(catalog)
    .mockRejectedValueOnce(new Error("sensitive diagnostic"))
    .mockResolvedValueOnce({ directory: catalog.directory, packages: [], issues: [] });
  render(<ExternalSkins {...props} scan={scan} />);
  await screen.findByRole("article");
  await refresh();
  expect((await screen.findByRole("alert")).textContent).toContain("仍显示上次扫描结果");
  expect(screen.queryByText("sensitive diagnostic")).toBeNull();
  expect(screen.getByRole("article")).toBeTruthy();
  await refresh();
  await screen.findByText("没有发现外部皮肤。");
  expect(screen.queryByRole("article")).toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("late old-host result cannot overwrite current catalog; busy scan cannot be duplicated", async () => {
  let resolve!: (result: SkinCatalog) => void;
  const scan = vi.fn(
    () =>
      new Promise<SkinCatalog>((done) => {
        resolve = done;
      }),
  );
  const mounted = render(<ExternalSkins {...props} scan={scan} />);
  fireEvent.click(screen.getByRole("button", { name: "正在扫描…" }));
  expect(scan).toHaveBeenCalledTimes(1);
  mounted.rerender(
    <ExternalSkins {...props} scan={async () => ({ ...catalog, packages: [], issues: [] })} />,
  );
  await screen.findByText("没有发现外部皮肤。");
  await act(async () => resolve(catalog));
  expect(screen.queryByRole("article")).toBeNull();
});

test("unavailable hosts and synchronous scan exceptions are handled", async () => {
  const mounted = render(<ExternalSkins {...props} />);
  expect((screen.getByRole("button", { name: "刷新皮肤" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  mounted.rerender(
    <ExternalSkins
      {...props}
      scan={() => {
        throw new Error("synthetic");
      }}
    />,
  );
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect((screen.getByRole("button", { name: "刷新皮肤" }) as HTMLButtonElement).disabled).toBe(
    false,
  );
});

test("manifest text is escaped and palette cannot inject CSS or resource URLs", async () => {
  const hostile = "</style><img src=x onerror=alert(1)>";
  render(
    <ExternalSkins
      {...props}
      scan={async () => ({
        ...catalog,
        packages: [
          {
            ...catalog.packages[0],
            name: hostile,
            candidate: {
              dark: {
                surface: "red;} body{display:none}",
                text: "url(https://invalid.example)",
                accent: "#123456",
                showSelectedBar: false,
              },
              light: {},
            },
          },
        ],
      })}
    />,
  );
  const card = await screen.findByRole("article", { name: hostile });
  expect(card.querySelector("img[src=x]")).toBeNull();
  // Colours reach the preview only as normalized `--cand-*` values; anything else is dropped, never written into a rule.
  expect(drawn(card, "--accent-strong")).toBe("#123456");
  expect(drawn(card, "--cand-bg")).toBe("");
  expect(drawn(card, "--cand-text")).toBe("");
  const css = previewCss(card);
  expect(css).toContain("display:none");
  expect(css).not.toContain("body");
  expect(css).not.toContain("url(");
});

test("an import host lists the imported skin without a manual refresh", async () => {
  const scan = vi.fn().mockResolvedValue({ directory: "/skins", packages: [], issues: [] });
  const openDirectory = vi.fn().mockResolvedValue(undefined);
  render(<ExternalSkins {...props} importsSkin openDirectory={openDirectory} scan={scan} />);
  expect(screen.getByText(/选中包含 skin.toml 的皮肤文件夹/)).toBeTruthy();
  expect(screen.queryByText(/复制到下面的目录/)).toBeNull();
  await screen.findByText("没有发现外部皮肤。");
  expect(scan).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "导入皮肤" }));
  await waitFor(() => expect(scan).toHaveBeenCalledTimes(2));
  expect(openDirectory).toHaveBeenCalledTimes(1);
});

test("an import that fails does not rescan", async () => {
  const scan = vi.fn().mockResolvedValue({ directory: "/skins", packages: [], issues: [] });
  const openDirectory = vi.fn().mockRejectedValue(new Error("synthetic"));
  render(<ExternalSkins {...props} importsSkin openDirectory={openDirectory} scan={scan} />);
  fireEvent.click(screen.getByRole("button", { name: "导入皮肤" }));
  expect((await screen.findByRole("alert")).textContent).toBe("导入皮肤失败，请重试。");
  // Only the scan the list makes as it mounts.
  expect(scan).toHaveBeenCalledTimes(1);
});

test("a host that draws one layout judges skins by it, not by the shared setting", async () => {
  const vertical = {
    ...initial,
    preferences: { ...initial.preferences, candidate_layout: "vertical" },
  };
  const client = (host?: { platform: string; fixed_candidate_layout?: "horizontal" }) => ({
    load: async () => vertical as Snapshot,
    save: vi.fn(),
    scanSkinCatalog: vi.fn().mockResolvedValue(catalog),
    host: host as never,
  });
  const mounted = render(
    <SettingsPage
      initialPage="skin"
      client={client({ platform: "ios", fixed_candidate_layout: "horizontal" })}
    />,
  );
  const card = await screen.findByRole("article", { name: /Sample skin/ });
  expect(within(card).queryByText(/当前布局或明暗模式不受支持/)).toBeNull();
  mounted.unmount();

  render(<SettingsPage initialPage="skin" client={client()} />);
  const desktopCard = await screen.findByRole("article", { name: /Sample skin/ });
  expect(within(desktopCard).getByText(/当前布局或明暗模式不受支持/)).toBeTruthy();
});

test("a host without a floating toolbar previews only the candidate window and reads no toolbar css", async () => {
  const readToolbarCss = vi.fn().mockResolvedValue(null);
  render(
    <ExternalSkins
      {...props}
      scan={async () => ({
        ...catalog,
        packages: [{ ...catalog.packages[0], toolbarStylesheet: "toolbar.css" }],
      })}
      toolbarPreview={false}
    />,
  );
  const card = await screen.findByRole("article", { name: "Sample skin" });
  expect(card.querySelectorAll("[data-skin-stage]")).toHaveLength(2);
  // No reader was passed either, yet the card must not claim the toolbar styles are unsupported: there is no toolbar for them to style.
  expect(within(card).queryByText("当前宿主不支持外部工具栏样式。")).toBeNull();
  cleanup();
  render(
    <ExternalSkins
      {...props}
      scan={async () => ({
        ...catalog,
        packages: [{ ...catalog.packages[0], toolbarStylesheet: "toolbar.css" }],
      })}
      readToolbarCss={readToolbarCss}
      toolbarPreview={false}
    />,
  );
  await screen.findByRole("article", { name: "Sample skin" });
  expect(readToolbarCss).not.toHaveBeenCalled();
});

test("the Linux skin page describes the candidate window only", async () => {
  const readSkinToolbarCss = vi.fn().mockResolvedValue(null);
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: async () => initial,
        save: vi.fn(),
        readSkinToolbarCss,
        host: { platform: "linux" } as never,
        scanSkinCatalog: async () => ({
          ...catalog,
          packages: [{ ...catalog.packages[0], toolbarStylesheet: "toolbar.css" }],
        }),
      }}
    />,
  );
  const external = await screen.findByRole("article", { name: /Sample skin/ });
  // Both Linux hosts present the toolbar as an input method menu, which no skin styles.
  expect(
    screen.getByText("选择候选窗使用的主题；明暗预览仅影响当前卡片，不修改设置。"),
  ).toBeTruthy();
  // Every page is mounted at once; the toolbar page itself still names the toolbar.
  expect(within(external.closest("fieldset")!).queryByText(/悬浮工具栏/)).toBeNull();
  const builtin = screen.getByRole("article", { name: "夜青" });
  expect(within(builtin).getByText("深色候选窗与键盘")).toBeTruthy();
  expect(builtin.querySelectorAll("[data-skin-stage]")).toHaveLength(2);
  expect(external.querySelectorAll("[data-skin-stage]")).toHaveLength(2);
  expect(readSkinToolbarCss).not.toHaveBeenCalled();
});

test("the Windows skin page keeps the toolbar preview", async () => {
  render(
    <SettingsPage
      initialPage="skin"
      client={{ load: async () => initial, save: vi.fn(), host: { platform: "windows" } as never }}
    />,
  );
  await screen.findByRole("button", { name: "保存设置" });
  expect(
    screen.getByText("选择候选窗和悬浮工具栏使用的主题；明暗预览仅影响当前卡片，不修改设置。"),
  ).toBeTruthy();
  const builtin = screen.getByRole("article", { name: "夜青" });
  expect(within(builtin).getByText("深色候选窗、悬浮工具栏与键盘")).toBeTruthy();
  expect(builtin.querySelectorAll("[data-skin-stage]")).toHaveLength(3);
});

// The keys a msime-skins package adds: its own decoration image and alignment, a background, a card radius and a toolbar palette.
const styledCatalog: SkinCatalog = {
  ...catalog,
  packages: [
    {
      ...catalog.packages[0],
      preview: "preview.png",
      decorationTopDip: 104,
      decorationWidthDip: 96,
      decorationImage: "assets/character.png",
      decorationAlign: "left",
      cornerRadiusDip: 12,
      background: { image: "assets/background.png", fit: "stretch", opacity: 0.35 },
      toolbar: {
        cornerRadiusDip: 8,
        dark: { background: "#141B33", handle: "#5B9BFF", icon: "red;background:url(x)" },
        light: { background: "#F4F8FF" },
      },
    },
  ],
};

test("a styled package draws its decoration image, alignment, background, radius and toolbar", async () => {
  const readImage = vi.fn().mockResolvedValue(imageData);
  render(
    <ExternalSkins
      {...props}
      activeTheme="dark"
      toolbarPreview
      scan={async () => styledCatalog}
      readImage={readImage}
    />,
  );
  const card = await screen.findByRole("article");
  await waitFor(() => expect(card.querySelectorAll("img.skin-background-image")).toHaveLength(2));
  // The decoration comes from `decorationImage`, not the preview.
  expect(readImage).toHaveBeenCalledWith("sample", "assets/character.png");
  expect(readImage).toHaveBeenCalledWith("sample", "assets/background.png");
  expect(readImage).not.toHaveBeenCalledWith("sample", "preview.png");
  for (const layout of ["horizontal", "vertical"]) {
    const image = card.querySelector<HTMLImageElement>(
      `[data-preview-layout="${layout}"] .container > img.skin-background-image`,
    )!;
    expect(image.getAttribute("src")).toBe(skinImageUrl(imageData));
    expect(image.style.objectFit).toBe("fill");
    expect(image.style.opacity).toBe("0.35");
  }
  const preview = card.querySelector<HTMLElement>("[data-skin-preview]")!;
  expect(preview.dataset.decorationAlign).toBe("left");
  expect(drawn(card, "--msime-skin-radius")).toBe("12px");
  expect(drawn(card, "--msime-toolbar-radius")).toBe("8px");
  expect(drawn(card, "--msime-toolbar-background")).toBe("#141B33");
  expect(drawn(card, "--msime-toolbar-handle")).toBe("#5B9BFF");
  // A value that is not a colour never reaches the style.
  expect(drawn(card, "--msime-toolbar-icon")).toBe("");
  expect(preview.getAttribute("style")).not.toContain("url(");
  fireEvent.click(within(card).getByRole("button", { name: "预览浅色" }));
  expect(drawn(card, "--msime-toolbar-background")).toBe("#F4F8FF");
  expect(drawn(card, "--msime-toolbar-handle")).toBe("");
});

test("a background that fails to load keeps the plain card and says so", async () => {
  const readImage = vi.fn(async (_id: string, relative: string) => {
    if (relative === "assets/background.png") throw new Error("synthetic");
    return imageData;
  });
  render(<ExternalSkins {...props} scan={async () => styledCatalog} readImage={readImage} />);
  const card = await screen.findByRole("article");
  expect(await within(card).findByText(/皮肤图片加载失败/)).toBeTruthy();
  expect(card.querySelector("img.skin-background-image")).toBeNull();
  expect(card.querySelectorAll("img.skin-decoration-image")).toHaveLength(2);
});

test("the geometry stylesheet aligns the decoration and clips the background to the card", () => {
  const card = utilityCss("skin-card-preview").replace(/\s+/g, "");
  const geometry = geometryCss.replace(/\s+/g, "");
  // Aligned against the card with its own padding: left at pad_x, centre on the card.
  expect(geometry).toMatch(
    /\[data-decoration-align="left"\]\.skin-decoration-image,[^{]*\{left:var\(--msime-candidate-pad-x,0px\);object-position:leftbottom;/,
  );
  expect(geometry).toMatch(
    /\[data-decoration-align="center"\]\.skin-decoration-image,[^{]*\{left:max\(0px,calc\(50%-var\(--msime-skin-decoration-width,0px\)\/2\)\);object-position:centerbottom;/,
  );
  // pad_x / pad_y are the card's own padding, one name for both.
  expect(card).toContain("--msime-candidate-pad-x:1px;--msime-candidate-pad-y:2px;");
  expect(card).toContain("--msime-candidate-pad-x:2px;--msime-candidate-pad-y:2px;");
  expect(
    card.match(/padding:var\(--msime-candidate-pad-y\)var\(--msime-candidate-pad-x\)/g),
  ).toHaveLength(2);
  expect(card).toContain("border-radius:var(--msime-skin-radius,6px)");
  expect(card).toContain(
    ".container:has(>.skin-background-image){position:relative;overflow:hidden",
  );
  expect(card).toContain("--ftb-radius:var(--msime-toolbar-radius,8px)");
});

test("发布到社区 appears only when the host can publish candidate skins", async () => {
  const scan = vi.fn().mockResolvedValue(catalog);
  const plain = render(<ExternalSkins {...props} scan={scan} />);
  const card = await screen.findByRole("article", { name: "Sample skin" });
  expect(within(card).queryByRole("button", { name: "发布到社区" })).toBeNull();
  plain.unmount();

  const onPublish = vi.fn();
  render(<ExternalSkins {...props} scan={scan} onPublish={onPublish} />);
  const publishable = await screen.findByRole("article", { name: "Sample skin" });
  fireEvent.click(within(publishable).getByRole("button", { name: "发布到社区" }));
  expect(onPublish).toHaveBeenCalledExactlyOnceWith("sample");
});

test("the theme page opens the candidate publish dialog outside the settings fieldset", async () => {
  const save = vi.fn();
  const communityCandidateSkins = {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn(),
    preview: vi.fn(),
    install: vi.fn(),
    packPreview: vi.fn().mockResolvedValue({
      suggestedName: "Sample skin",
      license: { code: null, assets: "CC0-1.0", source: null },
      fileCount: 1,
      size: 4096,
    }),
    publish: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        load: async () => initial,
        save,
        scanSkinCatalog: vi.fn().mockResolvedValue(catalog),
        communityCandidateSkins,
      }}
    />,
  );
  // A desktop host with only the candidate-skin client still lists 社区.
  expect(await screen.findByRole("button", { name: "社区" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  const card = await screen.findByRole("article", { name: "Sample skin" });
  fireEvent.click(within(card).getByRole("button", { name: "发布到社区" }));
  const dialog = await screen.findByRole("dialog", { name: "发布候选窗皮肤" });
  expect(dialog.closest("fieldset")).toBeNull();
  await waitFor(() => expect(communityCandidateSkins.packPreview).toHaveBeenCalledWith("sample"));
  const name = await within(dialog).findByRole("textbox", { name: "发布皮肤名称" });
  // The dialog renders inside the settings form, and jsdom never performs implicit submission, so what keeps Enter from saving the draft in a browser is the keydown's default being prevented.
  expect(dialog.closest("form")).not.toBeNull();
  expect(fireEvent.keyDown(name, { key: "Enter" })).toBe(false);
  expect(save).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
  expect(screen.queryByRole("dialog", { name: "发布候选窗皮肤" })).toBeNull();
  expect(save).not.toHaveBeenCalled();
});
