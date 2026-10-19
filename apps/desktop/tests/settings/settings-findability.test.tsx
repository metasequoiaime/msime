// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";

const snapshot: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    voice_input: {
      enabled: true,
      asr_provider: "doubao",
      language: "zh-CN",
      asr_token: "synthetic-secret-only",
    },
  },
};

beforeEach(() => {
  Element.prototype.scrollIntoView = vi.fn();
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

async function mount(devices = true) {
  const listDevices = vi.fn().mockResolvedValue([]);
  const save = vi.fn().mockResolvedValue(snapshot);
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: async () => snapshot,
        save,
        host: testHost({
          platform: "macos",
          voice_capture_devices: devices,
          candidate_font_controls: true,
        }),
        ...(devices ? { listVoiceCaptureDevices: listDevices } : {}),
      }}
    />,
  );
  await settingsFormReady();
  return { listDevices, save };
}

function search(query: string) {
  const field = screen.getByRole("searchbox", { name: "搜索设置" });
  fireEvent.change(field, { target: { value: query } });
  return field;
}

test("从别页搜索麦克风，切到录音行为并聚焦；只在该标签读取设备", async () => {
  const { listDevices, save } = await mount();
  expect(listDevices).not.toHaveBeenCalled();
  search("麦克风");
  const result = within(screen.getByRole("listbox", { name: "设置搜索结果" })).getByRole("option", {
    name: /麦克风/,
  });
  expect(result.textContent).toContain("语音输入 › 录音行为");
  fireEvent.click(result);
  expect(screen.getByRole("tabpanel", { name: "录音行为" })).toBeTruthy();
  const microphone = screen.getByRole("combobox", { name: "麦克风" });
  await waitFor(() => expect(document.activeElement).toBe(microphone));
  expect(listDevices).toHaveBeenCalledTimes(1);
  expect(save).not.toHaveBeenCalled();
  expect(screen.getByRole("searchbox")).toHaveProperty("value", "");
});

test("搜索服务高级行，打开下钻与折叠；再次搜索录音选项返回主标签", async () => {
  await mount();
  const field = search("资源 ID");
  fireEvent.keyDown(field, { key: "Enter" });
  const service = screen.getByRole("region", { name: "服务配置" });
  const resource = within(service).getByRole("textbox", { name: "Doubao 资源 ID" });
  expect(resource.closest("details")?.open).toBe(true);
  await waitFor(() => expect(document.activeElement).toBe(resource));
  const next = search("按住录音");
  fireEvent.keyDown(next, { key: "Enter" });
  expect(screen.queryByRole("region", { name: "服务配置" })).toBeNull();
  expect(screen.getByRole("tabpanel", { name: "识别与输入" })).toBeTruthy();
  await waitFor(() =>
    expect(document.activeElement).toBe(screen.getByRole("combobox", { name: "按住录音" })),
  );
});

test("搜索不读取凭据值，也不列出无宿主能力的设备设置", async () => {
  await mount(false);
  search("synthetic-secret-only");
  expect(within(screen.getByRole("listbox")).queryAllByRole("option")).toHaveLength(0);
  expect(within(screen.getByRole("listbox")).getByRole("status").textContent).toContain(
    "没有匹配的设置",
  );
  search("麦克风");
  expect(within(screen.getByRole("listbox")).queryAllByRole("option")).toHaveLength(0);
});

test("设置结果支持方向键与回车，组词期间保持查询与当前页", async () => {
  await mount();
  const field = search("录音");
  const results = within(screen.getByRole("listbox")).getAllByRole("option");
  expect(results.length).toBeGreaterThan(1);
  const first = field.getAttribute("aria-activedescendant");
  fireEvent.keyDown(field, { key: "ArrowDown", isComposing: true });
  fireEvent.keyDown(field, { key: "Enter", keyCode: 229 });
  expect(field.getAttribute("aria-activedescendant")).toBe(first);
  expect(field).toHaveProperty("value", "录音");
  fireEvent.keyDown(field, { key: "ArrowDown" });
  expect(field.getAttribute("aria-activedescendant")).toBe(results[1].id);
  fireEvent.keyDown(field, { key: "ArrowUp" });
  expect(field.getAttribute("aria-activedescendant")).toBe(first);
  fireEvent.keyDown(field, { key: "Escape" });
  expect(screen.queryByRole("listbox")).toBeNull();
});

test("搜索候选字体直接切换隐藏的字体标签并聚焦选项", async () => {
  await mount();
  const field = search("候选字体");
  fireEvent.keyDown(field, { key: "Enter" });
  expect(screen.getByRole("tabpanel", { name: "字体与大小" })).toBeTruthy();
  const group = screen.getByRole("radiogroup", { name: "候选字体" });
  await waitFor(() => expect(group.contains(document.activeElement)).toBe(true));
});
