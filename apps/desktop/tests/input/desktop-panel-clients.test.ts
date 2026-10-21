import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ClipboardHistoryClient, Snapshot } from "@msime/ui";
import { createDesktopPanelClients } from "../../src/input/desktop-panel-clients";
import { createDesktopClipboardClient } from "../../src/input/desktop-clipboard-client";

const bridge = vi.hoisted(() => ({
  drag: vi.fn(async () => {}),
  resize: vi.fn(async (_edge: string) => {}),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    startDragging: bridge.drag,
    startResizeDragging: bridge.resize,
  })),
}));

beforeEach(() => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  vi.mocked(listen).mockResolvedValue(() => {});
});
afterEach(() => {
  vi.resetAllMocks();
  vi.restoreAllMocks();
});

function setup(clipboardHistory?: boolean) {
  const snapshot: Snapshot = {
    format_version: 1,
    revision: 7,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
      clipboard_history: clipboardHistory,
    },
  };
  const preferences = {
    load: vi.fn(async () => snapshot),
    save: vi.fn(async (_revision: number, _preferences: Snapshot["preferences"]) => snapshot),
  };
  const cloudDictionary = { close: vi.fn(async () => {}), request: vi.fn(async () => ({})) };
  const clients = createDesktopPanelClients(
    preferences,
    cloudDictionary,
    createDesktopClipboardClient(),
  );
  return { clients, preferences, snapshot, cloudDictionary };
}

test("装配时不启动原生操作，云词库保留传入对象的身份", () => {
  const { clients, preferences, cloudDictionary } = setup();
  expect(clients.cloudDictionary).toBe(cloudDictionary);
  expect(invoke).not.toHaveBeenCalled();
  expect(listen).not.toHaveBeenCalled();
  expect(getCurrentWindow).not.toHaveBeenCalled();
  expect(preferences.load).not.toHaveBeenCalled();
});

test.each([
  ["keyboard", "keyboard-panel"],
  ["handwriting", "handwriting-panel"],
  ["voice", "voice-panel"],
  ["cloudClipboard", "cloud-clipboard-panel"],
  ["emoji", "emoji-panel"],
] as const)("%s 关闭自己的窗口", async (name, label) => {
  await setup().clients[name].close();
  expect(invoke).toHaveBeenCalledExactlyOnceWith("close_panel", { label });
});

test.each(["voice", "cloudClipboard", "emoji"] as const)(
  "%s 共用目标记录、发送和复制命令",
  async (name) => {
    const client = setup().clients[name];
    await client.rememberInputTarget!();
    await client.sendText!("合成发送文字");
    await client.copyText!("合成复制文字");
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["remember_input_target"],
      ["send_text", { text: "合成发送文字" }],
      ["copy_text", { text: "合成复制文字" }],
    ]);
  },
);

test("键盘与手写保持窗口拖动和各自的输入命令", async () => {
  const { keyboard, handwriting } = setup().clients;
  await keyboard.beginWindowDrag!();
  await handwriting.beginWindowDrag!();
  expect(bridge.drag).toHaveBeenCalledTimes(2);
  await keyboard.openVoice!();
  await keyboard.rememberInputTarget!();
  const key = {
    virtual_key: 65,
    shift: true,
    modifiers: { ctrl: false, alt: false, win: false },
    include_sticky_modifiers: true,
  };
  await keyboard.sendKey!(key);
  const ink = { language: "zh-CN", strokes: [{ points: [{ x: 1, y: 2 }] }] };
  vi.mocked(invoke).mockResolvedValueOnce({ candidates: ["合成候选"] });
  await expect(handwriting.recognizeHandwriting!(ink)).resolves.toEqual({
    candidates: ["合成候选"],
  });
  await handwriting.submitHandwritingCandidate!("合成候选");
  await handwriting.copyHandwritingCandidate!("合成候选");
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["open_voice_panel"],
    ["remember_input_target"],
    ["send_key", { request: key }],
    ["recognize_handwriting", { request: ink }],
    ["submit_handwriting_candidate", { candidate: "合成候选" }],
    ["copy_text", { text: "合成候选" }],
  ]);
});

test("表情面板提供标题拖动和八方向原生缩放，不改输入目标", async () => {
  const { emoji } = setup().clients;
  await emoji.beginWindowDrag!();
  expect(bridge.drag).toHaveBeenCalledOnce();
  for (const edge of ["n", "s", "e", "w", "ne", "nw", "se", "sw"] as const) {
    await emoji.resizeWindow!(edge);
  }
  expect(bridge.resize.mock.calls.map(([edge]) => edge)).toEqual([
    "North",
    "South",
    "East",
    "West",
    "NorthEast",
    "NorthWest",
    "SouthEast",
    "SouthWest",
  ]);
  expect(invoke).not.toHaveBeenCalled();
});

test("语音桥解包事件并继续按活跃请求过滤", async () => {
  const { voice } = setup().clients;
  const requestId = "11111111-1111-4111-8111-111111111111";
  vi.spyOn(crypto, "randomUUID").mockReturnValue(requestId);
  let finish!: (result: { text: string }) => void;
  vi.mocked(invoke).mockReturnValueOnce(new Promise((resolve) => (finish = resolve)));
  const receive = vi.fn();
  const stop = vi.fn();
  vi.mocked(listen).mockResolvedValueOnce(stop);
  expect(await voice.onVoiceUpdate!(receive)).toBe(stop);
  expect(listen).toHaveBeenCalledWith("voice-update", expect.any(Function));
  const recognize = voice.recognizeVoice!("zh-CN");
  expect(invoke).toHaveBeenCalledWith("recognize_voice", {
    request: { language: "zh-CN", request_id: requestId },
  });
  const publish = vi.mocked(listen).mock.calls[0][1];
  const update = { request_id: requestId, text: "合成语音", final: false, level: 0.5 };
  publish({ event: "voice-update", id: 1, payload: { ...update, request_id: "synthetic-stale" } });
  expect(receive).not.toHaveBeenCalled();
  publish({ event: "voice-update", id: 1, payload: update });
  expect(receive).toHaveBeenCalledExactlyOnceWith(update);
  await voice.stopVoice!();
  expect(invoke).toHaveBeenCalledWith("stop_voice", { requestId });
  finish({ text: "合成最终文字" });
  await expect(recognize).resolves.toEqual({ text: "合成最终文字" });
  publish({ event: "voice-update", id: 1, payload: { ...update, final: true } });
  expect(receive).toHaveBeenCalledOnce();
  expect(voice.maxSubmitBytes).toBe(4096);
});

test("语音提交与云剪贴板能力和请求沿用原命令", async () => {
  const { voice, cloudClipboard } = setup().clients;
  vi.mocked(invoke).mockResolvedValueOnce("ja-JP");
  await expect(voice.loadVoiceLanguage!()).resolves.toBe("ja-JP");
  await voice.sendVoiceText!("合成语音提交");
  vi.mocked(invoke).mockResolvedValueOnce(false);
  await expect(cloudClipboard.canSendText!()).resolves.toBe(false);
  const action = { operation: "add", text: "合成云剪贴板" } as const;
  vi.mocked(invoke).mockResolvedValueOnce({ enabled: true });
  await expect(cloudClipboard.request(action)).resolves.toEqual({ enabled: true });
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["voice_input_language"],
    ["send_voice_text", { text: "合成语音提交" }],
    ["cloud_clipboard_can_send_text"],
    ["cloud_clipboard_request", { action }],
  ]);
});

test("表情目录透传，剪贴板读取和同步只投影文字", async () => {
  const { emoji } = setup().clients;
  const catalog = { emoji: [], kaomoji: [], symbols: [], unavailable: ["symbols"] };
  vi.mocked(invoke).mockResolvedValueOnce(catalog);
  await expect(emoji.loadCatalog!()).resolves.toBe(catalog);
  const entries = [
    { text: "合成第一条", timestampMs: 1, pinned: true },
    { text: "合成第二条", timestampMs: 2, pinned: false },
  ];
  vi.mocked(invoke).mockResolvedValueOnce(entries).mockResolvedValueOnce(entries);
  await expect(emoji.clipboard!.list!()).resolves.toEqual(["合成第一条", "合成第二条"]);
  await expect(emoji.clipboard!.sync!()).resolves.toEqual(["合成第一条", "合成第二条"]);
  await emoji.clipboard!.copy!("合成第一条");
  await emoji.clipboard!.remove!("合成第二条");
  await emoji.clipboard!.clear!();
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["load_emoji_catalog"],
    ["list_clipboard_history"],
    ["sync_clipboard_history"],
    ["copy_text", { text: "合成第一条" }],
    ["remove_clipboard_history", { text: "合成第二条" }],
    ["clear_clipboard_history"],
  ]);
});

test("表情面板使用注入的剪贴板客户端，并透传失败", async () => {
  const { preferences, cloudDictionary } = setup();
  const entries = [{ text: "合成注入记录", timestampMs: 3, pinned: true }];
  const failure = new Error("合成同步失败");
  const clipboard: Required<ClipboardHistoryClient> = {
    list: vi.fn(async () => entries),
    sync: vi.fn(async () => {
      throw failure;
    }),
    copy: vi.fn(async () => {}),
    remove: vi.fn(async () => {}),
    clear: vi.fn(async () => {}),
    setPinned: vi.fn(async () => {}),
  };
  const { emoji } = createDesktopPanelClients(preferences, cloudDictionary, clipboard);
  await expect(emoji.clipboard!.list!()).resolves.toEqual(["合成注入记录"]);
  await expect(emoji.clipboard!.sync!()).rejects.toBe(failure);
  await emoji.clipboard!.copy!("合成复制");
  await emoji.clipboard!.remove!("合成删除");
  await emoji.clipboard!.clear!();
  expect(clipboard.copy).toHaveBeenCalledExactlyOnceWith("合成复制");
  expect(clipboard.remove).toHaveBeenCalledExactlyOnceWith("合成删除");
  expect(clipboard.clear).toHaveBeenCalledOnce();
  expect(entries).toEqual([{ text: "合成注入记录", timestampMs: 3, pinned: true }]);
  expect(invoke).not.toHaveBeenCalled();
});

test.each([undefined, false, true])(
  "剪贴板启用状态为 %s 时保留偏好修订与其他字段",
  async (enabled) => {
    const { clients, preferences, snapshot } = setup(enabled);
    const clipboard = clients.emoji.clipboard!;
    await expect(clipboard.isEnabled!()).resolves.toBe(enabled ?? false);
    await clipboard.enable!();
    expect(preferences.load).toHaveBeenCalledTimes(2);
    if (enabled) expect(preferences.save).not.toHaveBeenCalled();
    else
      expect(preferences.save).toHaveBeenCalledExactlyOnceWith(7, {
        ...snapshot.preferences,
        clipboard_history: true,
      });
    expect(snapshot.preferences.clipboard_history).toBe(enabled);
  },
);

test("剪贴板依次订阅历史和偏好变更，并按原顺序释放", async () => {
  const released: string[] = [];
  vi.mocked(listen)
    .mockResolvedValueOnce(() => released.push("history"))
    .mockResolvedValueOnce(() => released.push("preferences"));
  const changed = vi.fn();
  const stop = await setup().clients.emoji.clipboard!.onChanged!(changed);
  expect(vi.mocked(listen).mock.calls.map(([name]) => name)).toEqual([
    "clipboard-history-changed",
    "preferences-changed",
  ]);
  for (const [name, receive] of vi.mocked(listen).mock.calls)
    receive({ event: name, id: 1, payload: {} });
  expect(changed).toHaveBeenCalledTimes(2);
  expect(released).toEqual([]);
  stop!();
  expect(released).toEqual(["history", "preferences"]);
});

test.each([0, 1])("第 %s 条剪贴板订阅失败时保留原错误并清理已注册监听", async (index) => {
  const failure = new Error("合成订阅失败");
  const stop = vi.fn();
  if (index) vi.mocked(listen).mockResolvedValueOnce(stop);
  vi.mocked(listen).mockRejectedValueOnce(failure);
  await expect(setup().clients.emoji.clipboard!.onChanged!(() => {})).rejects.toBe(failure);
  expect(listen).toHaveBeenCalledTimes(index + 1);
  expect(stop).toHaveBeenCalledTimes(index);
});
