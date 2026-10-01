// @vitest-environment jsdom
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { ComponentProps } from "react";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  within,
  waitFor,
} from "@testing-library/react";
import {
  ExternalSkinCard,
  selectedBarCss,
  useSkinPreviewAssets,
  useSkinCatalog,
} from "../../../../packages/ui/src/skin/external-skins";
import * as settingsStyle from "../../../../packages/ui/src/settings/settings-style";
import { themeCatalog } from "../../../../packages/ui/src/theme/global-theme";
import {
  SettingsPage,
  type ExternalSkin,
  type SettingsClient,
  type SkinCatalog,
  type Snapshot,
} from "@msime/ui";
import { utilityCss } from "../support/utility-css";

const geometryCss = utilityCss("external-skin-decorated");
import { skinImageUrl, type SkinImage } from "../../../../packages/ui/src/skin/skin-image";
import desktopConfig from "../../src-tauri/tauri.conf.json";
import * as fontPreparation from "../../../../packages/ui/src/skin/toolbar-fonts";
import { useSelectedBarPalette } from "../../../../packages/ui/src/skin/skin-palette";

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
const sample = catalog.packages[0];
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

// One package card on its own, as the theme carousel draws it.
const cardProps = {
  selected: false,
  layout: "horizontal",
  onSelect: vi.fn(),
  revision: 1,
  activeTheme: "dark" as const,
  toolbarPreview: true,
};
function packageCard(
  skin: ExternalSkin,
  props: Partial<ComponentProps<typeof ExternalSkinCard>> = {},
) {
  return <ExternalSkinCard {...cardProps} skin={skin} {...props} />;
}

// The theme page, where each package is a card of the carousel and the directory is the 外部皮肤 row of the 更多皮肤 group.
function openSkinPage(client: Partial<SettingsClient> = {}) {
  return render(
    <SettingsPage
      initialPage="skin"
      client={{ load: async () => initial, save: vi.fn(), ...client }}
    />,
  );
}
async function skinRow(): Promise<HTMLElement> {
  const title = await screen.findByText("外部皮肤", { selector: "[data-row-title]" });
  return title.closest("div")!;
}
// The row's group, which also holds the directory alerts and the ignored-folder list.
async function skinGroup(): Promise<HTMLElement> {
  return (await skinRow()).parentElement!;
}
function carousel(): HTMLElement {
  return screen.getByRole("region", { name: "主题列表" });
}

test("the theme page keeps the 外部皮肤 row on a host with a community page", async () => {
  openSkinPage({
    scanSkinCatalog: vi.fn().mockResolvedValue(catalog),
    openSkinDirectory: vi.fn(),
    communityCandidateSkins: {} as NonNullable<SettingsClient["communityCandidateSkins"]>,
  });
  expect(await screen.findByRole("article", { name: sample.name })).not.toBeNull();
  expect(await skinRow()).not.toBeNull();
  expect(screen.getByRole("button", { name: "刷新皮肤" })).not.toBeNull();
});

test("external skin selected-bar flag emits a scoped hide rule", () => {
  expect(selectedBarCss("scope", { showSelectedBar: false })).toEqual([
    ".scope .first::before{display:none !important}",
  ]);
  expect(selectedBarCss("scope", { showSelectedBar: true })).toEqual([]);
  expect(selectedBarCss("scope", null)).toEqual([]);
});

test("selected-bar palette hook installs and removes its scoped sheet", () => {
  const { result, rerender, unmount } = renderHook(
    ({ hideBar }) => useSelectedBarPalette("scope", hideBar),
    { initialProps: { hideBar: true } },
  );
  expect(result.current).toBe(false);
  expect(document.adoptedStyleSheets).toHaveLength(1);
  expect(document.adoptedStyleSheets[0].cssRules[0].cssText).toContain("display: none");

  rerender({ hideBar: false });
  expect(result.current).toBe(false);
  expect(document.adoptedStyleSheets).toHaveLength(0);
  unmount();
});

test("shared preview assets reset decode fallback when the preview revision changes", async () => {
  const readImage = vi.fn().mockResolvedValue(imageData);
  const { result, rerender } = renderHook(
    ({ revision }) =>
      useSkinPreviewAssets(readImage, { id: "sample", background: null }, "preview.png", revision),
    { initialProps: { revision: 0 } },
  );

  await waitFor(() => expect(result.current.image?.url).toContain("data:image/png"));
  act(() => result.current.onImageError());
  expect(result.current.decodeFailed).toBe(true);

  rerender({ revision: 1 });
  await waitFor(() => expect(result.current.decodeFailed).toBe(false));
  expect(readImage).toHaveBeenCalledWith("sample", "preview.png");
});
// The page scans once as it mounts; a manual refresh is clicked once that scan has settled and the button is back.
async function refresh() {
  fireEvent.click(await within(await skinRow()).findByRole("button", { name: "刷新皮肤" }));
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
          packages: [{ ...sample, toolbarStylesheet: "toolbar.css" }],
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
            packages: [{ ...sample, toolbarStylesheet: "toolbar.css" }],
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
    packageCard({
      ...sample,
      candidate: {
        dark: { surface: "#123456", showSelectedBar: false },
        light: { surface: "#abcdef" },
      },
    }),
  );
  const card = screen.getByRole("article");
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
  render(packageCard({ ...sample, candidate: { dark: { showSelectedBar: false }, light: {} } }));
  await screen.findByText("当前浏览器无法隐藏皮肤的选中条，其余配色照常预览。");
  expect(screen.getByRole("article").querySelector("style")).toBeNull();
});

const imageCatalog: SkinCatalog = {
  ...catalog,
  packages: [
    {
      ...sample,
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
  const readSkinImage = vi.fn().mockResolvedValue(imageData);
  const mounted = openSkinPage({ scanSkinCatalog: async () => imageCatalog, readSkinImage });
  expect(readSkinImage).not.toHaveBeenCalled();
  const card = await screen.findByRole("article", { name: "Sample skin" });
  await waitFor(() => expect(card.querySelectorAll("img.skin-decoration-image")).toHaveLength(2));
  expect(readSkinImage).toHaveBeenCalledExactlyOnceWith("sample", "images/top.png");
  expect(card.querySelector("img.skin-decoration-image")?.getAttribute("src")).toBe(
    skinImageUrl(imageData),
  );
  fireEvent.click(within(card).getByRole("button", { name: "预览浅色" }));
  expect(readSkinImage).toHaveBeenCalledTimes(1);
  await refresh();
  await waitFor(() => expect(readSkinImage).toHaveBeenCalledTimes(2));
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
  const readSkinImage = vi
    .fn()
    .mockRejectedValueOnce(new Error("private diagnostic"))
    .mockResolvedValue(imageData);
  openSkinPage({ scanSkinCatalog: async () => imageCatalog, readSkinImage });
  await screen.findByText("皮肤图片加载失败，保留基础预览。可刷新皮肤重试。");
  expect(screen.queryByText("private diagnostic")).toBeNull();
  await refresh();
  const card = screen.getByRole("article", { name: "Sample skin" });
  await waitFor(() => expect(card.querySelector("img.skin-decoration-image")).not.toBeNull());
  fireEvent.error(card.querySelector("img.skin-decoration-image")!);
  expect(card.querySelector("img.skin-decoration-image")).toBeNull();
  expect(card.querySelectorAll(".candidate .container")).toHaveLength(2);
  await refresh();
  await waitFor(() => expect(card.querySelectorAll("img.skin-decoration-image")).toHaveLength(2));
});

test("old image response cannot replace current resource after catalog refresh", async () => {
  let finish!: (image: SkinImage) => void;
  const readSkinImage = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise<SkinImage>((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValue({ ...imageData, bytes: [2] });
  openSkinPage({ scanSkinCatalog: async () => imageCatalog, readSkinImage });
  await waitFor(() => expect(readSkinImage).toHaveBeenCalledTimes(1));
  await refresh();
  const card = screen.getByRole("article", { name: "Sample skin" });
  await waitFor(() =>
    expect(card.querySelector("img.skin-decoration-image")?.getAttribute("src")).toContain("Ag=="),
  );
  await act(async () => finish(imageData));
  expect(card.querySelector("img.skin-decoration-image")?.getAttribute("src")).toContain("Ag==");
});

test("images are not requested without decoration and optional hosts remain usable", async () => {
  const readImage = vi.fn();
  const mounted = render(
    packageCard(
      { ...imageCatalog.packages[0], decorationTopDip: 0, decorationWidthDip: 0 },
      { readImage },
    ),
  );
  screen.getByRole("article");
  expect(readImage).not.toHaveBeenCalled();
  mounted.rerender(packageCard(imageCatalog.packages[0]));
  await screen.findByText("当前宿主不支持皮肤图片预览。");
});

test("decorated previews preserve upstream geometry in both layouts without decorating toolbar", async () => {
  render(
    packageCard({ ...sample, minWidthDip: 280.5, decorationTopDip: 32.5, decorationWidthDip: 150 }),
  );
  const card = screen.getByRole("article");
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
  // The two candidate stages share a row; the card's last stage of its own is the toolbar, which draws no candidate.
  expect(
    card.querySelector("[data-skin-preview] > [data-skin-stage]:last-child .containerParent"),
  ).toBeNull();
  const candidateRow = card.querySelector("[data-skin-preview] > :first-child")!;
  expect(candidateRow.querySelectorAll(":scope > [data-skin-stage] .containerParent")).toHaveLength(
    2,
  );
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
    render(packageCard({ ...sample, minWidthDip: Infinity, decorationTopDip, decorationWidthDip }));
    const card = screen.getByRole("article");
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
          ...sample,
          decorationTopDip: 500,
          decorationWidthDip: 1000,
          minWidthDip: 1000,
        },
        { ...sample, id: "plain", name: "Plain" },
      ],
    })
    .mockResolvedValue(catalog);
  openSkinPage({ scanSkinCatalog: scan });
  const card = await screen.findByRole("article", { name: "Sample skin" });
  expect(card.querySelectorAll(".containerParent")).toHaveLength(2);
  expect(
    screen.getByRole("article", { name: "Plain" }).querySelector(".containerParent"),
  ).toBeNull();
  await refresh();
  await waitFor(() => expect(screen.queryByRole("article", { name: "Plain" })).toBeNull());
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
    packageCard({
      ...sample,
      base: "paper",
      themes: ["light"],
      candidate: {
        dark: { surface: "#123456", text: "#112233" },
        light: { accent: "#aa0000" },
      },
    }),
  );
  const card = screen.getByRole("article");
  // Paper fixes the light mode; its surface and text show through where the light palette is silent.
  expect(drawn(card, "--cand-bg")).toBe("#F7F5F0");
  expect(drawn(card, "--cand-text")).toBe("#1A1E1B");
  expect(drawn(card, "--accent-strong")).toBe("#AA0000");
  expect(drawn(card, "--cand-selected")).toBe("#AA000024");
  expect(previewCss(card)).toBe("");
});

test("open directory is explicit, path-free and independent of scanning and selection", async () => {
  const openSkinDirectory = vi.fn().mockResolvedValue(undefined);
  const scanSkinCatalog = vi.fn().mockResolvedValue(catalog);
  const save = vi.fn();
  openSkinPage({ openSkinDirectory, scanSkinCatalog, save });
  const card = await screen.findByRole("article", { name: "Sample skin" });
  expect(openSkinDirectory).not.toHaveBeenCalled();
  const row = await skinRow();
  fireEvent.click(within(row).getByRole("button", { name: "打开目录" }));
  await within(row).findByRole("button", { name: "打开目录" });
  expect(openSkinDirectory).toHaveBeenCalledWith();
  // Only the scan the page makes as it mounts; opening the folder does not rescan.
  expect(scanSkinCatalog).toHaveBeenCalledTimes(1);
  expect(within(card).getByRole("switch").getAttribute("aria-checked")).toBe("false");
  expect(save).not.toHaveBeenCalled();
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
  const mounted = openSkinPage();
  expect(
    (within(await skinRow()).getByRole("button", { name: "打开目录" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  mounted.unmount();
  const openSkinDirectory = vi
    .fn()
    .mockImplementationOnce(() => {
      throw new Error("private diagnostic");
    })
    .mockResolvedValue(undefined);
  openSkinPage({ openSkinDirectory });
  const row = await skinRow();
  fireEvent.click(within(row).getByRole("button", { name: "打开目录" }));
  const group = await skinGroup();
  expect((await within(group).findByRole("alert")).textContent).toBe("无法打开皮肤目录，请重试。");
  expect(screen.queryByText("private diagnostic")).toBeNull();
  fireEvent.click(within(row).getByRole("button", { name: "打开目录" }));
  await within(row).findByRole("button", { name: "打开目录" });
  expect(within(group).queryByRole("alert")).toBeNull();
  expect(openSkinDirectory).toHaveBeenCalledTimes(2);
});

test("opening deduplicates requests and ignores late failures after host replacement", async () => {
  let reject!: (error: Error) => void;
  const openDirectory = vi.fn(
    () =>
      new Promise<void>((_resolve, fail) => {
        reject = fail;
      }),
  );
  const { result, rerender } = renderHook(
    ({ open }: { open: () => Promise<void> }) => useSkinCatalog(undefined, open, false),
    { initialProps: { open: openDirectory as () => Promise<void> } },
  );
  act(() => void result.current.openFolder());
  expect(result.current.opening).toBe(true);
  act(() => void result.current.openFolder());
  expect(openDirectory).toHaveBeenCalledTimes(1);
  rerender({ open: async () => {} });
  await act(async () => reject(new Error("synthetic")));
  expect(result.current.openFailed).toBe(false);
  expect(result.current.opening).toBe(false);
});

test("the page opening scans the catalog and its row shows the directory and diagnostics", async () => {
  let finish!: (result: SkinCatalog) => void;
  const scan = vi.fn(
    () =>
      new Promise<SkinCatalog>((resolve) => {
        finish = resolve;
      }),
  );
  openSkinPage({ scanSkinCatalog: scan });
  const row = await skinRow();
  // The scan starts in an effect once the page has rendered.
  await waitFor(() =>
    expect(within(row).getByRole("status").textContent).toBe("正在读取皮肤目录。"),
  );
  const busy = within(row).getByRole("button", { name: "正在扫描…" }) as HTMLButtonElement;
  expect(busy.disabled).toBe(true);
  // The busy button cannot start a second scan.
  fireEvent.click(busy);
  expect(scan).toHaveBeenCalledTimes(1);
  await act(async () => finish(catalog));
  const card = await screen.findByRole("article", { name: "Sample skin" });
  expect(scan).toHaveBeenCalledExactlyOnceWith();
  expect(within(row).getByText(catalog.directory)).toBeTruthy();
  expect(within(row).getByRole("status").textContent).toBe("");
  expect(within(card).getByText("sample · v1 · Example")).toBeTruthy();
  const group = await skinGroup();
  expect(within(group).getByText("已忽略 1 个无效皮肤目录")).toBeTruthy();
  expect(within(group).getByText("Bad：invalid manifest")).toBeTruthy();
  expect(card.querySelectorAll("[data-skin-stage]")).toHaveLength(3);
  // The packages are carousel cards now; the separate section below the page is gone.
  expect(screen.queryByRole("region", { name: "外部皮肤" })).toBeNull();
});

test("external skins join the theme carousel after the built-in themes", async () => {
  openSkinPage({
    scanSkinCatalog: async () => ({
      ...catalog,
      packages: [sample, { ...sample, id: "second", name: "Second skin" }],
    }),
  });
  await screen.findByRole("article", { name: "Second skin" });
  const slides = within(carousel())
    .getAllByRole("article")
    .map((article) => article.getAttribute("aria-label"));
  expect(slides).toEqual([
    ...themeCatalog.map((entry) => entry.title),
    "Sample skin",
    "Second skin",
  ]);
  expect(within(carousel()).getByRole("button", { name: "查看Second skin" })).toBeTruthy();
  const total = themeCatalog.length + 2;
  expect(within(carousel()).getByText(`1 / ${total}`)).toBeTruthy();
});

test("a host without a catalog scanner has no package cards and says so in the row", async () => {
  openSkinPage();
  const row = await skinRow();
  expect(within(row).getByRole("status").textContent).toBe("当前宿主不支持扫描外部皮肤。");
  expect(within(carousel()).getAllByRole("article")).toHaveLength(themeCatalog.length);
  expect(within(carousel()).getByText(`1 / ${themeCatalog.length}`)).toBeTruthy();
});

test("an external card is styled as the built-in theme cards are", async () => {
  openSkinPage({ scanSkinCatalog: async () => catalog });
  const card = await screen.findByRole("article", { name: "Sample skin" });
  const builtin = screen.getByRole("article", { name: "夜青" });
  expect(card.className).toBe(builtin.className);
  const toggle = within(card).getByRole("switch");
  expect(toggle.className).toBe(within(builtin).getByRole("switch").className);
  expect(toggle.className).toBe(settingsStyle.skinSwitch(false));
  expect(toggle.classList.contains("skin-selection-switch")).toBe(false);
  expect(toggle.firstElementChild?.className).toBe(settingsStyle.skinSwitchKnob(false));
  // The title carries the drawn mode as the built-in titles do.
  expect(card.querySelector("[data-skin-card-header] span")?.textContent).toBe(
    "Sample skin（深色）",
  );
  expect(within(card).getByRole("button", { name: "预览浅色" }).className).toBe(
    settingsStyle.skinPreviewSwitch,
  );
});

test("choosing a package makes it the custom theme's skin, and the 自定义 card drops it", async () => {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...initial,
    revision: 4,
    preferences,
  }));
  openSkinPage({
    save,
    scanSkinCatalog: async () => ({ ...catalog, packages: [{ ...sample, base: "paper" }] }),
  });
  const card = await screen.findByRole("article", { name: "Sample skin" });
  const custom = screen.getByRole("article", { name: "自定义" });
  fireEvent.click(within(card).getByRole("switch"));
  expect(within(card).getByRole("switch").getAttribute("aria-checked")).toBe("true");
  expect(within(card).getByText("使用中")).toBeTruthy();
  expect(card.className).toBe(settingsStyle.skinCard(true));
  // The custom theme is in use, but through the package's card.
  expect(within(custom).getByRole("switch").getAttribute("aria-checked")).toBe("false");
  expect(within(custom).queryByText("使用中")).toBeNull();
  expect(within(custom).getByText("外部皮肤、候选颜色与自定义键盘")).toBeTruthy();
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(3, {
    ...initial.preferences,
    global_theme: "custom",
    custom_theme: { base: "paper", candidate_skin: "sample" },
  });
  fireEvent.click(within(custom).getByRole("switch"));
  expect(within(custom).getByRole("switch").getAttribute("aria-checked")).toBe("true");
  expect(within(card).getByRole("switch").getAttribute("aria-checked")).toBe("false");
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenLastCalledWith(4, {
      ...initial.preferences,
      global_theme: "custom",
      custom_theme: { base: "paper", candidate_skin: null },
    }),
  );
});

test("the carousel opens on the package card when the custom theme draws a package", async () => {
  openSkinPage({
    load: async () => ({
      ...initial,
      preferences: {
        ...initial.preferences,
        global_theme: "custom",
        custom_theme: { candidate_skin: "sample" },
      },
    }),
    scanSkinCatalog: async () => catalog,
  });
  const card = await screen.findByRole("article", { name: "Sample skin" });
  const count = themeCatalog.length + 1;
  await waitFor(() =>
    expect(
      within(carousel())
        .getByRole("button", { name: "查看Sample skin" })
        .getAttribute("aria-current"),
    ).toBe("true"),
  );
  expect(within(carousel()).getByText(`${count} / ${count}`)).toBeTruthy();
  expect(within(card).getByRole("switch").getAttribute("aria-checked")).toBe("true");
  expect(within(card).getByText("使用中")).toBeTruthy();
  expect(
    within(screen.getByRole("article", { name: "自定义" }))
      .getByRole("switch")
      .getAttribute("aria-checked"),
  ).toBe("false");
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
  saveSettingsNow();
  expect(save).toHaveBeenCalledWith(
    3,
    expect.objectContaining({
      global_theme: "custom",
      custom_theme: expect.objectContaining({ base: "system", candidate_skin: "sample" }),
    }),
  );
});

test("light-only skin compatibility follows actual theme, not card override", async () => {
  const lightOnly = { ...sample, themes: ["light"] };
  const onSelect = vi.fn();
  const view = render(packageCard(lightOnly, { onSelect, activeTheme: "dark" }));
  const toggle = screen.getByRole("switch") as HTMLButtonElement;
  expect(toggle.disabled).toBe(true);
  view.rerender(packageCard(lightOnly, { onSelect, activeTheme: "light" }));
  expect(toggle.disabled).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "预览深色" }));
  expect(toggle.disabled).toBe(false);
  fireEvent.click(toggle);
  expect(onSelect).toHaveBeenCalledExactlyOnceWith("sample", "system");
  view.rerender(packageCard(lightOnly, { onSelect, activeTheme: "dark" }));
  expect(toggle.disabled).toBe(true);
  expect(
    view.container.querySelector("[data-skin-preview]")?.getAttribute("data-preview-theme"),
  ).toBe("light");
});

test("a package over a built-in base is drawn in that base's mode whatever the host mode", async () => {
  const night = { ...sample, base: "night" as const, themes: ["light"] };
  const onSelect = vi.fn();
  const view = render(packageCard(night, { onSelect, activeTheme: "light" }));
  const card = screen.getByRole("article", { name: "Sample skin" });
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
  view.rerender(packageCard(night, { onSelect, activeTheme: "dark" }));
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
  const mounted = render(packageCard(sample, { onSelect, layout: "vertical" }));
  const toggle = screen.getByRole("switch", { name: "Sample skin" }) as HTMLButtonElement;
  expect(toggle.disabled).toBe(true);
  fireEvent.click(toggle);
  fireEvent.click(screen.getByRole("button", { name: "预览浅色" }));
  expect(toggle.disabled).toBe(true);
  expect(onSelect).not.toHaveBeenCalled();
  mounted.unmount();
  render(packageCard({ ...sample, themes: ["light"] }));
  expect((screen.getByRole("switch") as HTMLButtonElement).disabled).toBe(true);
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
  openSkinPage({ scanSkinCatalog: scan });
  await screen.findByRole("article", { name: "Sample skin" });
  const group = await skinGroup();
  await refresh();
  expect((await within(group).findByRole("alert")).textContent).toContain("仍显示上次扫描结果");
  expect(screen.queryByText("sensitive diagnostic")).toBeNull();
  expect(screen.getByRole("article", { name: "Sample skin" })).toBeTruthy();
  await refresh();
  await within(group).findByText("没有发现外部皮肤。");
  expect(screen.queryByRole("article", { name: "Sample skin" })).toBeNull();
  expect(within(group).queryByRole("alert")).toBeNull();
});

test("late old-host result cannot overwrite current catalog; busy scan cannot be duplicated", async () => {
  let resolve!: (result: SkinCatalog) => void;
  const scan = vi.fn(
    () =>
      new Promise<SkinCatalog>((done) => {
        resolve = done;
      }),
  );
  const { result, rerender } = renderHook(
    ({ scan }: { scan: () => Promise<SkinCatalog> }) => useSkinCatalog(scan, undefined, false),
    { initialProps: { scan: scan as () => Promise<SkinCatalog> } },
  );
  expect(result.current.busy).toBe(true);
  act(() => void result.current.refresh());
  expect(scan).toHaveBeenCalledTimes(1);
  rerender({ scan: async () => ({ ...catalog, packages: [], issues: [] }) });
  await waitFor(() => expect(result.current.catalog?.packages).toEqual([]));
  const revision = result.current.revision;
  await act(async () => resolve(catalog));
  expect(result.current.catalog?.packages).toEqual([]);
  expect(result.current.revision).toBe(revision);
});

test("unavailable hosts and synchronous scan exceptions are handled", async () => {
  const mounted = openSkinPage();
  expect(
    (within(await skinRow()).getByRole("button", { name: "刷新皮肤" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  mounted.unmount();
  openSkinPage({
    scanSkinCatalog: () => {
      throw new Error("synthetic");
    },
  });
  const group = await skinGroup();
  expect((await within(group).findByRole("alert")).textContent).toBe("读取皮肤目录失败，请重试。");
  expect(
    (within(await skinRow()).getByRole("button", { name: "刷新皮肤" }) as HTMLButtonElement)
      .disabled,
  ).toBe(false);
});

test("manifest text is escaped and palette cannot inject CSS or resource URLs", async () => {
  const hostile = "</style><img src=x onerror=alert(1)>";
  render(
    packageCard({
      ...sample,
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
    }),
  );
  const card = screen.getByRole("article", { name: hostile });
  expect(card.querySelector("img[src=x]")).toBeNull();
  // Colours reach the preview only as normalized `--cand-*` values; anything else is dropped, never written into a rule.
  expect(drawn(card, "--accent-strong")).toBe("#123456");
  expect(drawn(card, "--cand-bg")).toBe("");
  expect(drawn(card, "--cand-text")).toBe("");
  await waitFor(() => expect(previewCss(card)).toContain("display:none"));
  const css = previewCss(card);
  expect(css).not.toContain("body");
  expect(css).not.toContain("url(");
});

test("an import host lists the imported skin without a manual refresh", async () => {
  const scan = vi.fn().mockResolvedValue({ directory: "/skins", packages: [], issues: [] });
  const openDirectory = vi.fn().mockResolvedValue(undefined);
  openSkinPage({
    host: { platform: "harmony", skin_directory_import: true } as never,
    openSkinDirectory: openDirectory,
    scanSkinCatalog: scan,
  });
  const row = await skinRow();
  expect(within(row).getByText(/选中包含 skin.toml 的皮肤文件夹/)).toBeTruthy();
  expect(within(row).queryByText(/复制到下面的目录/)).toBeNull();
  await within(row).findByText("没有发现外部皮肤。");
  expect(scan).toHaveBeenCalledTimes(1);
  fireEvent.click(within(row).getByRole("button", { name: "导入皮肤" }));
  await waitFor(() => expect(scan).toHaveBeenCalledTimes(2));
  expect(openDirectory).toHaveBeenCalledTimes(1);
});

test("an import that fails does not rescan", async () => {
  const scan = vi.fn().mockResolvedValue({ directory: "/skins", packages: [], issues: [] });
  const openDirectory = vi.fn().mockRejectedValue(new Error("synthetic"));
  openSkinPage({
    host: { platform: "harmony", skin_directory_import: true } as never,
    openSkinDirectory: openDirectory,
    scanSkinCatalog: scan,
  });
  const row = await skinRow();
  await within(row).findByText("没有发现外部皮肤。");
  fireEvent.click(within(row).getByRole("button", { name: "导入皮肤" }));
  const group = await skinGroup();
  expect((await within(group).findByRole("alert")).textContent).toBe("导入皮肤失败，请重试。");
  // Only the scan the page makes as it mounts.
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
  const withToolbarCss = { ...sample, toolbarStylesheet: "toolbar.css" };
  render(packageCard(withToolbarCss, { toolbarPreview: false }));
  const card = screen.getByRole("article", { name: "Sample skin" });
  expect(card.querySelectorAll("[data-skin-stage]")).toHaveLength(2);
  // No reader was passed either, yet the card must not claim the toolbar styles are unsupported: there is no toolbar for them to style.
  expect(within(card).queryByText("当前宿主不支持外部工具栏样式。")).toBeNull();
  cleanup();
  render(packageCard(withToolbarCss, { readToolbarCss, toolbarPreview: false }));
  screen.getByRole("article", { name: "Sample skin" });
  await act(async () => {});
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
          packages: [{ ...sample, toolbarStylesheet: "toolbar.css" }],
        }),
      }}
    />,
  );
  const external = await screen.findByRole("article", { name: /Sample skin/ });
  // Both Linux hosts present the toolbar as an input method menu, which no skin styles.
  expect(
    screen.getByText("选择候选窗口使用的主题；明暗预览仅影响当前卡片，不修改设置。"),
  ).toBeTruthy();
  // Every page is mounted at once; the toolbar page itself still names the toolbar.
  expect(within(external.closest("fieldset")!).queryByText(/悬浮工具栏/)).toBeNull();
  const builtin = screen.getByRole("article", { name: "夜青" });
  expect(within(builtin).getByText("深色候选窗口与键盘")).toBeTruthy();
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
  await settingsFormReady();
  expect(
    screen.getByText("选择候选窗口和悬浮工具栏使用的主题；明暗预览仅影响当前卡片，不修改设置。"),
  ).toBeTruthy();
  const builtin = screen.getByRole("article", { name: "夜青" });
  expect(within(builtin).getByText("深色候选窗口、悬浮工具栏与键盘")).toBeTruthy();
  expect(builtin.querySelectorAll("[data-skin-stage]")).toHaveLength(3);
});

// The keys a msime-skins package adds: its own decoration image and alignment, a background, a card radius and a toolbar palette.
const styledSkin: ExternalSkin = {
  ...sample,
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
};

test("a styled package draws its decoration image, alignment, background, radius and toolbar", async () => {
  const readImage = vi.fn().mockResolvedValue(imageData);
  render(packageCard(styledSkin, { activeTheme: "dark", toolbarPreview: true, readImage }));
  const card = screen.getByRole("article");
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
  render(packageCard(styledSkin, { readImage }));
  const card = screen.getByRole("article");
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
  const plain = render(packageCard(sample));
  const card = screen.getByRole("article", { name: "Sample skin" });
  expect(within(card).queryByRole("button", { name: "发布到社区" })).toBeNull();
  plain.unmount();

  const onPublish = vi.fn();
  render(packageCard(sample, { onPublish }));
  const publishable = screen.getByRole("article", { name: "Sample skin" });
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
    addPreview: vi.fn(),
    addLicense: vi.fn(),
    publish: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
    setVisibility: vi.fn(),
    setCategory: vi.fn(),
    sync: vi.fn(),
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
  await waitFor(() =>
    expect(communityCandidateSkins.packPreview).toHaveBeenCalledWith("sample", "public"),
  );
  const name = await within(dialog).findByRole("textbox", { name: "发布皮肤名称" });
  // The dialog renders inside the settings form, and jsdom never performs implicit submission, so what keeps Enter from saving the draft in a browser is the keydown's default being prevented.
  expect(dialog.closest("form")).not.toBeNull();
  expect(fireEvent.keyDown(name, { key: "Enter" })).toBe(false);
  expect(save).not.toHaveBeenCalled();
  fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
  expect(screen.queryByRole("dialog", { name: "发布候选窗皮肤" })).toBeNull();
  expect(save).not.toHaveBeenCalled();
});
