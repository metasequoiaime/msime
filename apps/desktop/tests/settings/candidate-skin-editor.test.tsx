// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type HostCapabilities, type SkinCatalog, type Snapshot } from "@msime/ui";
import { CandidateSkinEditor } from "../../../../packages/ui/src/skin/candidate-skin-editor";
import type { CandidateSkinEditorClient } from "../../../../packages/ui/src/skin/candidate-skin-draft";
import type { ExternalSkin } from "../../../../packages/ui/src/skin/external-skins";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

const catalog: SkinCatalog = {
  directory: "/synthetic/skins",
  issues: [],
  packages: [
    {
      id: "sample",
      name: "Synthetic external",
      version: "1",
      base: "system",
      author: null,
      description: null,
      layouts: ["horizontal", "vertical"],
      themes: ["dark", "light"],
      minWidthDip: 0,
      decorationTopDip: 0,
      decorationWidthDip: 0,
      toolbarStylesheet: null,
      preview: "preview.png",
      candidate: { dark: { surface: "#123456" }, light: { surface: "#ABCDEF" } },
    },
  ],
};

function editorClient() {
  return {
    check: vi.fn<CandidateSkinEditorClient["check"]>(),
    create: vi.fn<CandidateSkinEditorClient["create"]>(async (request) => ({
      id: request.packageId,
      replaced: false,
    })),
  };
}

function renderSettings(
  platform: string,
  fields: Omit<Partial<HostCapabilities>, "platform"> = {},
  client: Partial<{ candidateSkinEditor: CandidateSkinEditorClient }> = {
    candidateSkinEditor: editorClient(),
  },
) {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(async (_revision, preferences) => ({ ...initial, preferences })),
        host: testHost({ platform, ...fields }),
        scanSkinCatalog: vi.fn().mockResolvedValue(catalog),
        ...client,
      }}
    />,
  );
}

test("the desktop skin page offers 制作皮肤 right after 本地皮肤 when the host can install skins", async () => {
  renderSettings("macos", { candidate_skin_editor: true });
  await settingsFormReady();
  expect(screen.getByRole("region", { name: "明暗" })).toBeTruthy();
  const tabs = screen.getByRole("tablist", { name: "皮肤来源" });
  expect(
    within(tabs)
      .getAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(["本地皮肤", "制作皮肤"]);
  fireEvent.click(within(tabs).getByRole("tab", { name: "制作皮肤" }));
  const panel = screen.getByRole("tabpanel", { name: "制作皮肤" });
  expect(within(panel).getByRole("textbox", { name: "皮肤名称" })).toBeTruthy();
  // 编辑器画在表单之外，表单里的皮肤页随之隐藏。
  expect(screen.queryByRole("region", { name: "明暗" })).toBeNull();
  expect(
    within(screen.getByRole("form", { name: "设置" })).queryByRole("textbox", { name: "皮肤名称" }),
  ).toBeNull();
});

test("the tab follows the capability, not the platform name", async () => {
  renderSettings("macos");
  await settingsFormReady();
  expect(screen.queryByRole("tab", { name: "制作皮肤" })).toBeNull();
  cleanup();

  renderSettings("linux", { candidate_skin_editor: true }, {});
  await settingsFormReady();
  expect(screen.queryByRole("tab", { name: "制作皮肤" })).toBeNull();
  cleanup();

  renderSettings("linux", { candidate_skin_editor: true });
  await settingsFormReady();
  expect(screen.getByRole("tab", { name: "制作皮肤" })).toBeTruthy();
  cleanup();

  // 手机外壳没有桌面的皮肤标签，即使宿主报了这一项。
  renderSettings("harmony", { candidate_skin_editor: true, mobile_settings: true });
  await settingsFormReady();
  expect(screen.queryByRole("tab", { name: "制作皮肤" })).toBeNull();
});

function renderEditor(
  client = editorClient(),
  props: Partial<Parameters<typeof CandidateSkinEditor>[0]> = {},
) {
  const onUse = vi.fn();
  const onShowLocal = vi.fn();
  const renderPreview = vi.fn(async () => [0x89, 0x50, 0x4e, 0x47]);
  render(
    <CandidateSkinEditor
      client={client}
      catalog={catalog}
      onUse={onUse}
      inUse={() => false}
      onShowLocal={onShowLocal}
      renderPreview={renderPreview}
      {...props}
    />,
  );
  return { client, onUse, onShowLocal, renderPreview };
}

test("saving sends the manifest and images through the host and offers to use the skin", async () => {
  const { client, onUse, renderPreview } = renderEditor();
  fireEvent.change(screen.getByRole("textbox", { name: "皮肤名称" }), {
    target: { value: "Autumn Leaves" },
  });
  expect((screen.getByRole("textbox", { name: "皮肤 ID" }) as HTMLInputElement).value).toBe(
    "autumn-leaves",
  );
  fireEvent.click(screen.getByRole("button", { name: "保存皮肤" }));
  await waitFor(() => expect(client.create).toHaveBeenCalledTimes(1));
  const [request, replace] = client.create.mock.calls[0];
  expect(replace).toBe(false);
  expect(request.packageId).toBe("autumn-leaves");
  expect(Object.keys(request.files)).toEqual(["preview.png"]);
  expect(request.manifest).toContain('id = "autumn-leaves"');
  expect(request.manifest).toContain('name = "Autumn Leaves"');
  expect(request.manifest).toContain('preview = "preview.png"');
  expect(renderPreview).toHaveBeenCalledTimes(1);
  expect(await screen.findByText(/已保存「Autumn Leaves」/)).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "立即使用" }));
  expect(onUse).toHaveBeenCalledWith("autumn-leaves", "system");

  // 再保存同一款只是更新它，不再询问。
  fireEvent.change(screen.getByRole("textbox", { name: "皮肤说明" }), {
    target: { value: "新的说明" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存修改" }));
  await waitFor(() => expect(client.create).toHaveBeenCalledTimes(2));
  expect(client.create.mock.calls[1][1]).toBe(true);
  expect(client.create.mock.calls[1][0].manifest).toContain('description = "新的说明"');
});

test("replacing an installed skin needs an explicit confirmation", async () => {
  const client = editorClient();
  client.create.mockImplementation(async (request, replace) => {
    if (!replace) throw { code: "candidate_skin_exists" };
    return { id: request.packageId, replaced: true };
  });
  renderEditor(client);
  fireEvent.change(screen.getByRole("textbox", { name: "皮肤名称" }), {
    target: { value: "Copy" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "皮肤 ID" }), {
    target: { value: "sample" },
  });
  expect(screen.getByText(/本地已有 ID 为“sample”的皮肤「Synthetic external」/)).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "保存皮肤" }));
  const dialog = await screen.findByRole("alertdialog", { name: "替换已有的皮肤？" });
  fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  expect(client.create).toHaveBeenCalledTimes(1);

  fireEvent.click(screen.getByRole("button", { name: "保存皮肤" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", { name: "替换" }),
  );
  await waitFor(() => expect(client.create).toHaveBeenCalledTimes(3));
  expect(client.create.mock.calls[2][1]).toBe(true);
  expect(await screen.findByText(/已替换「Copy」/)).toBeTruthy();
});

test("a refusal from the host is shown in plain Chinese", async () => {
  const client = editorClient();
  client.create.mockRejectedValue({ code: "candidate_skin_package", detail: "invalid background" });
  renderEditor(client);
  fireEvent.change(screen.getByRole("textbox", { name: "皮肤名称" }), {
    target: { value: "Broken" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存皮肤" }));
  expect((await screen.findByRole("alert")).textContent).toBe(
    "皮肤没有通过校验：背景图的设置无效：不透明度要在 0 到 1 之间。",
  );
});

test("problems the page can see stop the save and are listed", async () => {
  const { client } = renderEditor();
  fireEvent.click(screen.getByRole("button", { name: "保存皮肤" }));
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("请给皮肤起个名字。");
  expect(alert.textContent).toContain("请填写皮肤 ID。");
  expect(client.create).not.toHaveBeenCalled();

  fireEvent.change(screen.getByRole("textbox", { name: "皮肤 ID" }), {
    target: { value: "wechat" },
  });
  // 皮肤 ID 一行的说明和保存前的问题列表都说出原因。
  expect(screen.getAllByText("“wechat”是内置主题或外观的名字，请换一个 ID。")).toHaveLength(2);
});

test("the contrast readout warns below 4.5:1 as colours change", () => {
  renderEditor();
  expect(screen.getByText("文字对比度都不低于 4.5:1。")).toBeTruthy();
  const hex = screen.getByRole("textbox", { name: "候选文字（浅色）色值" });
  fireEvent.change(hex, { target: { value: "#F0F0F0" } });
  expect(screen.getByText("有 2 组文字对比度低于 4.5:1，可能看不清。")).toBeTruthy();
  expect(screen.getAllByText(/:1 偏低$/)).toHaveLength(2);
  // 写不完整的色值不改配色，失焦后恢复成当前颜色。
  fireEvent.change(hex, { target: { value: "#12" } });
  expect(hex.getAttribute("aria-invalid")).toBe("true");
  fireEvent.blur(hex);
  expect((hex as HTMLInputElement).value).toBe("#F0F0F0");
});

test("starting from a fixed-mode theme limits the skin to that mode", async () => {
  renderEditor();
  fireEvent.change(screen.getByRole("combobox", { name: "从这里开始" }), {
    target: { value: "theme:night" },
  });
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "载入" })));
  expect((screen.getByRole("combobox", { name: "底色" }) as HTMLSelectElement).value).toBe("night");
  expect(screen.getByText("底色是深色主题，这款皮肤只用在深色模式。")).toBeTruthy();
  expect(
    (screen.getByRole("textbox", { name: "窗口底色（深色）色值" }) as HTMLInputElement).value,
  ).toBe("#16262F");
  expect(screen.queryByRole("textbox", { name: "窗口底色（浅色）色值" })).toBeNull();
});

test("a Windows look asks the host for its palette, and leaving edits asks first", async () => {
  const client = editorClient();
  const template: ExternalSkin = {
    ...catalog.packages[0],
    id: "template",
    name: "template",
    candidate: {
      light: { surface: "#F7F7F7", accent: "#07C160", selected: "#07C160" },
      dark: { surface: "#151515" },
    },
    cornerRadiusDip: 5,
  };
  client.check.mockResolvedValue(template);
  renderEditor(client);
  fireEvent.change(screen.getByRole("textbox", { name: "皮肤名称" }), {
    target: { value: "Edited" },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "从这里开始" }), {
    target: { value: "look:wechat" },
  });
  fireEvent.click(screen.getByRole("button", { name: "载入" }));
  const dialog = await screen.findByRole("alertdialog", { name: "换一个起点？" });
  fireEvent.click(within(dialog).getByRole("button", { name: "换起点" }));
  await waitFor(() => expect(client.check).toHaveBeenCalledTimes(1));
  expect(client.check.mock.calls[0][0]).toMatchObject({ packageId: "template", files: {} });
  expect(client.check.mock.calls[0][0].manifest).toContain('base = "wechat"');
  await waitFor(() =>
    expect((screen.getByRole("combobox", { name: "底色" }) as HTMLSelectElement).value).toBe(
      "wechat",
    ),
  );
  expect((screen.getByRole("textbox", { name: "皮肤名称" }) as HTMLInputElement).value).toBe("");
  await waitFor(() =>
    expect(
      (screen.getByRole("textbox", { name: "选中背景（浅色）色值" }) as HTMLInputElement).value,
    ).toBe("#07C160"),
  );
});

test("duplicating an installed skin starts from a copy with its own id", async () => {
  renderEditor();
  fireEvent.change(screen.getByRole("combobox", { name: "从这里开始" }), {
    target: { value: "skin:sample" },
  });
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "载入" })));
  expect((screen.getByRole("textbox", { name: "皮肤名称" }) as HTMLInputElement).value).toBe(
    "Synthetic external 副本",
  );
  expect((screen.getByRole("textbox", { name: "皮肤 ID" }) as HTMLInputElement).value).toBe(
    "sample-copy",
  );
  expect(
    (screen.getByRole("textbox", { name: "窗口底色（浅色）色值" }) as HTMLInputElement).value,
  ).toBe("#ABCDEF");
});

test("the preview shows both modes and says why a mode is not drawn", () => {
  renderEditor();
  expect(screen.getByRole("group", { name: "浅色预览" })).toBeTruthy();
  expect(screen.getByRole("group", { name: "深色预览" })).toBeTruthy();
  fireEvent.click(screen.getByRole("checkbox", { name: "深色" }));
  expect(
    within(screen.getByRole("group", { name: "深色预览" })).getByText(
      "没有勾选深色模式，深色模式下不使用这款皮肤。",
    ),
  ).toBeTruthy();
  // 动效没打开时没有可播放的东西。
  expect((screen.getByRole("button", { name: "播放动效" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  fireEvent.click(screen.getByRole("switch", { name: "启用动效" }));
  expect((screen.getByRole("button", { name: "播放动效" }) as HTMLButtonElement).disabled).toBe(
    false,
  );
});
