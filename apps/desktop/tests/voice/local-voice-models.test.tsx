// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  LocalModelManager,
  SettingsPage,
  formatModelBytes,
  localModelErrorMessage,
  localModelImportErrorMessage,
  localModelInUse,
  localModelProgressPercent,
  validModelMirror,
  visibleLocalModels,
  type LocalVoiceModel,
  type LocalVoiceModelClient,
  type LocalVoiceModelProgress,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const root = "/Users/someone/Library/Application Support/msime/voice-models";

function model(overrides: Partial<LocalVoiceModel>): LocalVoiceModel {
  const id = overrides.id ?? "x-asr-zh-en-streaming";
  return {
    id,
    title: "中英流式",
    description: "边说边出字。",
    languages: ["zh", "en"],
    streaming: true,
    default: false,
    desktop_only: false,
    installed: false,
    path: `${root}/${id}`,
    installed_size: 200_000_000,
    archive_size: 133_895_136,
    memory: 500_000_000,
    license_spdx: "Apache-2.0",
    license_source: `https://example.com/${id}`,
    license_terms: "",
    license_notice: "模型按 Apache-2.0 许可分发。",
    hotwords: "native",
    import_files: [
      {
        name: `${id}.tar.bz2`,
        url: `https://example.com/releases/${id}.tar.bz2`,
        size: overrides.archive_size ?? 133_895_136,
      },
    ],
    ...overrides,
  };
}

const streaming = model({ id: "x-asr-zh-en-streaming", title: "中英流式", default: true });
const sense = model({
  id: "sense-voice-small",
  title: "快速整句",
  streaming: false,
  archive_size: 163_002_883,
  license_spdx: "LicenseRef-FunASR",
  license_terms: "https://example.com/funasr-terms",
  hotwords: "pinyin",
});
const nano = model({
  id: "fun-asr-nano",
  title: "高精度",
  streaming: false,
  desktop_only: true,
  archive_size: 841_730_611,
  memory: 1_500_000_000,
});

function fakeClient(models: LocalVoiceModel[]) {
  let current = models.map((entry) => ({ ...entry }));
  let emit: ((progress: LocalVoiceModelProgress) => void) | undefined;
  let finishInstall: ((path: string) => void) | undefined;
  let failInstall: ((error: unknown) => void) | undefined;
  const client: LocalVoiceModelClient = {
    list: vi.fn(async () => ({ models: current, default: streaming.id, root })),
    install: vi.fn(
      (id: string) =>
        new Promise<string>((resolve, reject) => {
          finishInstall = (path) => {
            current = current.map((entry) =>
              entry.id === id ? { ...entry, installed: true } : entry,
            );
            resolve(path);
          };
          failInstall = reject;
        }),
    ),
    cancel: vi.fn(async () => {
      failInstall?.({ code: "local_model_cancelled" });
      return true;
    }),
    remove: vi.fn(async (id: string) => {
      current = current.map((entry) => (entry.id === id ? { ...entry, installed: false } : entry));
    }),
    onProgress: vi.fn(async (listener: (progress: LocalVoiceModelProgress) => void) => {
      emit = listener;
      return () => {
        emit = undefined;
      };
    }),
  };
  return {
    client,
    emit: (progress: LocalVoiceModelProgress) => act(() => emit?.(progress)),
    finish: (path: string) => act(async () => finishInstall?.(path)),
  };
}

test("sizes read in decimal units", () => {
  expect(formatModelBytes(133_895_136)).toBe("134 MB");
  expect(formatModelBytes(1_500_000_000)).toBe("1.5 GB");
  expect(formatModelBytes(0)).toBe("未知");
});

test("a phone does not offer the desktop-only model unless it is the one in use", () => {
  const models = [streaming, sense, nano];
  expect(visibleLocalModels(models, false, "").map((entry) => entry.id)).toEqual([
    streaming.id,
    sense.id,
    nano.id,
  ]);
  expect(visibleLocalModels(models, true, "").map((entry) => entry.id)).toEqual([
    streaming.id,
    sense.id,
  ]);
  expect(visibleLocalModels(models, true, `${nano.path}/`).map((entry) => entry.id)).toContain(
    nano.id,
  );
});

test("the model in use is matched by its directory", () => {
  expect(localModelInUse(sense, ` ${sense.path}/ `)).toBe(true);
  expect(localModelInUse(sense, streaming.path)).toBe(false);
  expect(localModelInUse(sense, "")).toBe(false);
});

test("progress is a bounded whole percentage", () => {
  expect(localModelProgressPercent(undefined)).toBe(0);
  expect(localModelProgressPercent({ id: "a", stage: "download", downloaded: 1, total: 3 })).toBe(
    33,
  );
  expect(localModelProgressPercent({ id: "a", stage: "extract", downloaded: 9, total: 3 })).toBe(
    100,
  );
  expect(localModelProgressPercent({ id: "a", stage: "done", downloaded: 0, total: 3 })).toBe(100);
  expect(localModelProgressPercent({ id: "a", stage: "download", downloaded: 5, total: 0 })).toBe(
    0,
  );
});

test("a cancel the user asked for is not reported as a failure", () => {
  expect(localModelErrorMessage({ code: "local_model_cancelled" })).toBeNull();
  expect(localModelErrorMessage({ code: "local_model_network" })).toContain("镜像");
  expect(localModelErrorMessage(new Error("boom"))).toBe("操作失败，请重试。");
});

test("a mirror is empty or a complete https URL without credentials or query", () => {
  expect(validModelMirror("")).toBe(true);
  expect(validModelMirror("https://ghproxy.example.com")).toBe(true);
  expect(validModelMirror("https://ghproxy.example.com/prefix/")).toBe(true);
  expect(validModelMirror(`https://ghproxy.example.com/${"中".repeat(700)}`)).toBe(false);
  expect(validModelMirror("http://ghproxy.example.com")).toBe(false);
  expect(validModelMirror("https://")).toBe(false);
  expect(validModelMirror("https:///missing-host")).toBe(false);
  expect(validModelMirror("https://user:pass@ghproxy.example.com")).toBe(false);
  expect(validModelMirror("https://ghproxy.example.com/?token=synthetic")).toBe(false);
  expect(validModelMirror("https://ghproxy.example.com/#fragment")).toBe(false);
  expect(validModelMirror("https://a b")).toBe(false);
});

function renderManager(
  client: LocalVoiceModelClient,
  options: { mobile?: boolean; modelPath?: string; confirmed?: boolean } = {},
) {
  const onUse = vi.fn();
  const onRemoved = vi.fn();
  const confirm = vi.fn(async () => options.confirmed ?? true);
  const openExternalUrl = vi.fn(async () => undefined);
  render(
    <LocalModelManager
      client={client}
      mobile={options.mobile ?? false}
      modelPath={options.modelPath ?? ""}
      onUse={onUse}
      onRemoved={onRemoved}
      confirm={confirm}
      openExternalUrl={openExternalUrl}
    />,
  );
  return { onUse, onRemoved, confirm, openExternalUrl };
}

test("each model shows what it costs, its languages and its license", async () => {
  const { client } = fakeClient([streaming, sense, nano]);
  const { openExternalUrl } = renderManager(client);

  const card = within(await screen.findByRole("listitem", { name: "快速整句" }));
  expect(card.getByText(/语言：中文、英语/)).toBeTruthy();
  expect(card.getByText(/下载 163 MB/)).toBeTruthy();
  expect(card.getByText(/LicenseRef-FunASR/)).toBeTruthy();
  expect(card.getByText(/https:\/\/example.com\/funasr-terms/)).toBeTruthy();
  expect(card.queryByText("流式")).toBeNull();
  expect(within(screen.getByRole("listitem", { name: "中英流式" })).getByText("流式")).toBeTruthy();

  fireEvent.click(card.getByRole("button", { name: sense.license_source }));
  expect(openExternalUrl).toHaveBeenCalledWith(sense.license_source);
});

test("the desktop-only model is not offered on a phone", async () => {
  const { client } = fakeClient([streaming, sense, nano]);
  renderManager(client, { mobile: true });

  await screen.findByRole("listitem", { name: "中英流式" });
  expect(screen.queryByRole("listitem", { name: "高精度" })).toBeNull();
});

test("downloading shows progress and a first model is put to use", async () => {
  const fake = fakeClient([streaming, sense]);
  const { onUse } = renderManager(fake.client);

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  fireEvent.click(card.getByRole("button", { name: /下载（134 MB）/ }));
  expect(fake.client.install).toHaveBeenCalledWith(streaming.id);

  fake.emit({ id: streaming.id, stage: "download", downloaded: 66_947_568, total: 133_895_136 });
  const bar = card.getByRole("progressbar", { name: "中英流式 下载进度" }) as HTMLProgressElement;
  expect(bar.value).toBe(50);
  expect(card.getByText("下载中 50%")).toBeTruthy();

  await fake.finish(streaming.path);
  expect(onUse).toHaveBeenCalledWith(streaming.path);
  await waitFor(() => expect(card.getByRole("button", { name: "使用" })).toBeTruthy());
  expect(card.queryByRole("progressbar")).toBeNull();
});

test("a later download does not replace the model in use", async () => {
  const fake = fakeClient([{ ...streaming, installed: true }, sense]);
  const { onUse } = renderManager(fake.client, { modelPath: streaming.path });

  const card = within(await screen.findByRole("listitem", { name: "快速整句" }));
  fireEvent.click(card.getByRole("button", { name: /下载/ }));
  await fake.finish(sense.path);

  await waitFor(() => expect(card.getByRole("button", { name: "使用" })).toBeTruthy());
  expect(onUse).not.toHaveBeenCalled();
});

test("a model picked while the first download runs is not replaced when it finishes", async () => {
  const fake = fakeClient([streaming, { ...sense, installed: true }]);
  const onUse = vi.fn();
  const props = {
    client: fake.client,
    mobile: false,
    onUse,
    onRemoved: vi.fn(),
    confirm: vi.fn(async () => true),
  };
  const view = render(<LocalModelManager {...props} modelPath="" />);

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  fireEvent.click(card.getByRole("button", { name: /下载/ }));
  // The user picks the installed model meanwhile, and the page re-renders with it.
  view.rerender(<LocalModelManager {...props} modelPath={sense.path} />);
  await fake.finish(streaming.path);

  await waitFor(() => expect(card.getByRole("button", { name: "使用" })).toBeTruthy());
  expect(onUse).not.toHaveBeenCalled();
});

test("a previous client's model list cannot replace the active client after a host switch", async () => {
  const first = fakeClient([streaming]);
  let resolveFirst:
    | ((value: { models: LocalVoiceModel[]; default: string; root: string }) => void)
    | undefined;
  vi.mocked(first.client.list).mockImplementation(
    () =>
      new Promise((resolve) => {
        resolveFirst = resolve;
      }),
  );
  const second = fakeClient([{ ...sense, installed: true }]);
  const props = {
    mobile: false,
    onUse: vi.fn(),
    onRemoved: vi.fn(),
    confirm: vi.fn(async () => true),
  };
  const view = render(<LocalModelManager {...props} client={first.client} modelPath="" />);
  view.rerender(<LocalModelManager {...props} client={second.client} modelPath="" />);
  await screen.findByRole("listitem", { name: "快速整句" });
  resolveFirst?.({ models: [streaming], default: streaming.id, root });
  await act(async () => {
    await Promise.resolve();
  });
  expect(screen.queryByRole("listitem", { name: "中英流式" })).toBeNull();
  expect(screen.getByRole("listitem", { name: "快速整句" })).toBeTruthy();
});

test("clears the previous client's model list while the replacement loads", async () => {
  const first = fakeClient([streaming]);
  let resolveSecond!: (value: { models: LocalVoiceModel[]; default: string; root: string }) => void;
  const second = fakeClient([sense]);
  vi.mocked(second.client.list).mockImplementation(
    () =>
      new Promise((resolve) => {
        resolveSecond = resolve;
      }),
  );
  const props = {
    mobile: false,
    modelPath: "",
    onUse: vi.fn(),
    onRemoved: vi.fn(),
    confirm: vi.fn(async () => true),
  };
  const view = render(<LocalModelManager {...props} client={first.client} />);
  await screen.findByRole("listitem", { name: "中英流式" });

  view.rerender(<LocalModelManager {...props} client={second.client} />);
  expect(screen.queryByRole("listitem", { name: "中英流式" })).toBeNull();

  resolveSecond({ models: [sense], default: sense.id, root });
  await screen.findByRole("listitem", { name: "快速整句" });
});

test("returning to a model client ignores progress from its previous subscription", async () => {
  const first = fakeClient([streaming]);
  const second = fakeClient([streaming]);
  const listeners: ((progress: LocalVoiceModelProgress) => void)[] = [];
  const resolveSubscriptions: ((stop: () => void) => void)[] = [];
  vi.mocked(first.client.onProgress).mockImplementation((listener) => {
    listeners.push(listener);
    return new Promise((resolve) => resolveSubscriptions.push(resolve));
  });
  const props = {
    mobile: false,
    modelPath: "",
    onUse: vi.fn(),
    onRemoved: vi.fn(),
    confirm: vi.fn(async () => true),
  };
  const view = render(<LocalModelManager {...props} client={first.client} />);
  await screen.findByRole("listitem", { name: "中英流式" });
  view.rerender(<LocalModelManager {...props} client={second.client} />);
  view.rerender(<LocalModelManager {...props} client={first.client} />);

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  fireEvent.click(card.getByRole("button", { name: /下载/ }));
  act(() => {
    listeners[1]({ id: streaming.id, stage: "download", downloaded: 22, total: 100 });
    listeners[0]({ id: streaming.id, stage: "download", downloaded: 77, total: 100 });
  });
  expect(card.getByText("下载中 22%")).toBeTruthy();

  const stopOld = vi.fn();
  const stopCurrent = vi.fn();
  await act(async () => {
    resolveSubscriptions[0](stopOld);
    resolveSubscriptions[1](stopCurrent);
  });
  expect(stopOld).toHaveBeenCalledOnce();
  expect(stopCurrent).not.toHaveBeenCalled();
  await first.finish(streaming.path);
  view.unmount();
  expect(stopCurrent).toHaveBeenCalledOnce();
});

test("cancelling a download stops it without an error", async () => {
  const fake = fakeClient([streaming]);
  const { onUse } = renderManager(fake.client);

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  fireEvent.click(card.getByRole("button", { name: /下载/ }));
  fireEvent.click(await card.findByRole("button", { name: "取消下载" }));

  expect(fake.client.cancel).toHaveBeenCalledWith(streaming.id);
  expect(await screen.findByText("已取消下载「中英流式」。")).toBeTruthy();
  expect(card.getByRole("button", { name: /下载（134 MB）/ })).toBeTruthy();
  expect(onUse).not.toHaveBeenCalled();
});

test("a failed download says why", async () => {
  const fake = fakeClient([streaming]);
  vi.mocked(fake.client.install).mockRejectedValueOnce({ code: "local_model_checksum_mismatch" });
  renderManager(fake.client);

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  fireEvent.click(card.getByRole("button", { name: /下载/ }));

  expect(await screen.findByText(/校验不通过/)).toBeTruthy();
});

test("using an installed model hands over its directory and marks it", async () => {
  const fake = fakeClient([
    { ...streaming, installed: true },
    { ...sense, installed: true },
  ]);
  const { onUse } = renderManager(fake.client, { modelPath: streaming.path });

  const inUse = within(await screen.findByRole("listitem", { name: "中英流式" }));
  expect(inUse.getByText("正在使用")).toBeTruthy();
  expect((inUse.getByRole("button", { name: "使用中" }) as HTMLButtonElement).disabled).toBe(true);

  fireEvent.click(
    within(screen.getByRole("listitem", { name: "快速整句" })).getByRole("button", {
      name: "使用",
    }),
  );
  expect(onUse).toHaveBeenCalledWith(sense.path);
});

test("removing asks first and reports the removed model", async () => {
  const fake = fakeClient([{ ...streaming, installed: true }]);
  const { confirm, onRemoved } = renderManager(fake.client, { modelPath: streaming.path });

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  fireEvent.click(card.getByRole("button", { name: "删除" }));

  await waitFor(() => expect(fake.client.remove).toHaveBeenCalledWith(streaming.id));
  expect(confirm).toHaveBeenCalledWith(expect.objectContaining({ danger: true }));
  expect(onRemoved).toHaveBeenCalledWith(expect.objectContaining({ id: streaming.id }));
  await waitFor(() => expect(card.getByRole("button", { name: /下载/ })).toBeTruthy());
});

test("declining the removal keeps the model", async () => {
  const fake = fakeClient([{ ...streaming, installed: true }]);
  const { confirm, onRemoved } = renderManager(fake.client, { confirmed: false });

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  fireEvent.click(card.getByRole("button", { name: "删除" }));

  await waitFor(() => expect(confirm).toHaveBeenCalledTimes(1));
  expect(fake.client.remove).not.toHaveBeenCalled();
  expect(onRemoved).not.toHaveBeenCalled();
});

/** 带「从文件导入」的宿主：`picked` 是用户在选择器里的结果，`null` 表示关掉了选择器。 */
function importingClient(models: LocalVoiceModel[]) {
  const fake = fakeClient(models);
  let finishImport: ((path: string | null) => void) | undefined;
  let failImport: ((error: unknown) => void) | undefined;
  const importModel = vi.fn(
    (_id: string) =>
      new Promise<string | null>((resolve, reject) => {
        finishImport = resolve;
        failImport = reject;
      }),
  );
  const client: LocalVoiceModelClient = { ...fake.client, import: importModel };
  return {
    ...fake,
    client,
    importModel,
    picked: (path: string | null) => act(async () => finishImport?.(path)),
    failed: (error: unknown) => act(async () => failImport?.(error)),
  };
}

test("a host without a file picker offers no import", async () => {
  const fake = fakeClient([streaming]);
  renderManager(fake.client);

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  expect(card.queryByRole("button", { name: "从文件导入" })).toBeNull();
  expect(card.queryByText(/不联网安装/)).toBeNull();
});

test("importing lists the files to download, links them and puts a first model to use", async () => {
  const sensePack = {
    ...sense,
    import_files: [
      {
        name: "sense.tar.bz2",
        url: "https://example.com/releases/sense.tar.bz2",
        size: 163_002_883,
      },
      {
        name: "silero_vad.onnx",
        url: "https://example.com/releases/silero_vad.onnx",
        size: 643_854,
      },
    ],
  };
  const fake = importingClient([sensePack]);
  const { onUse, openExternalUrl } = renderManager(fake.client);

  const card = within(await screen.findByRole("listitem", { name: "快速整句" }));
  expect(card.getByText(/不联网安装/)).toBeTruthy();
  fireEvent.click(card.getByRole("button", { name: "silero_vad.onnx（644 KB）" }));
  expect(openExternalUrl).toHaveBeenCalledWith("https://example.com/releases/silero_vad.onnx");

  fireEvent.click(card.getByRole("button", { name: "从文件导入" }));
  expect(fake.importModel).toHaveBeenCalledWith(sense.id);
  // 选择器还开着时没有进度条，只有取消。
  expect(card.queryByRole("progressbar")).toBeNull();
  expect(card.getByRole("button", { name: "取消导入" })).toBeTruthy();

  fake.emit({ id: sense.id, stage: "import", downloaded: 81_501_442, total: 163_002_884 });
  expect(card.getByRole("progressbar", { name: "快速整句 导入进度" })).toBeTruthy();
  expect(card.getByText("导入中 50%")).toBeTruthy();

  await fake.picked(sense.path);
  expect(onUse).toHaveBeenCalledWith(sense.path);
  expect(await screen.findByText("「快速整句」已导入。")).toBeTruthy();
  expect(card.queryByRole("progressbar")).toBeNull();
});

test("closing the file picker changes nothing", async () => {
  const fake = importingClient([streaming]);
  const { onUse } = renderManager(fake.client);

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  fireEvent.click(card.getByRole("button", { name: "从文件导入" }));
  await fake.picked(null);

  await waitFor(() => expect(card.getByRole("button", { name: "从文件导入" })).toBeTruthy());
  expect(onUse).not.toHaveBeenCalled();
  expect(screen.queryByRole("status")).toBeNull();
});

test("an import missing a file says what to do", async () => {
  const fake = importingClient([streaming]);
  renderManager(fake.client);

  const card = within(await screen.findByRole("listitem", { name: "中英流式" }));
  fireEvent.click(card.getByRole("button", { name: "从文件导入" }));
  await fake.failed({ code: "local_model_import_missing" });

  expect(await screen.findByText(/缺少这个模型需要的文件/)).toBeTruthy();
});

test("import failures have their own wording and fall back to the download ones", () => {
  expect(localModelImportErrorMessage({ code: "local_model_checksum_mismatch" })).toContain(
    "重新下载后再导入",
  );
  expect(localModelImportErrorMessage({ code: "local_model_cancelled" })).toBeNull();
  expect(localModelImportErrorMessage({ code: "local_model_import_unreadable" })).toContain(
    "读取所选文件失败",
  );
  expect(localModelImportErrorMessage({ code: "local_model_io" })).toBe(
    localModelErrorMessage({ code: "local_model_io" }),
  );
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 2,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    voice_input: {
      enabled: true,
      language: "zh-CN",
      asr_provider: "local",
      asr_model_path: "",
      asr_model_mirror: "",
    },
  },
};

// 语音页把本地模型放进「识别服务配置」组：模型列表在组里，下载镜像和手动目录收在默认关闭的「更多选项」里。
test("the voice page keeps the local models in the service group with the mirror folded away", async () => {
  const fake = fakeClient([{ ...sense, installed: true }]);
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "windows" }),
        localVoiceModels: fake.client,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));

  const service = await screen.findByRole("region", { name: "识别服务配置" });
  expect(within(service).getByRole("group", { name: "本地识别模型" })).toBeTruthy();
  const more = within(service).getByText("更多选项").closest("details");
  expect(more).not.toBeNull();
  expect(more!.open).toBe(false);
  expect(more!.contains(screen.getByLabelText("模型下载镜像"))).toBe(true);
  expect(more!.contains(screen.getByLabelText("本地模型目录"))).toBe(true);
  // 折叠区统一叫「更多选项」，旧的「高级：」说法只留在嵌入式面板里。
  expect(screen.queryByText("高级：手动指定本地模型目录")).toBeNull();
});

test("the settings page picks a model and a mirror into the saved preferences", async () => {
  const fake = fakeClient([{ ...sense, installed: true }]);
  const save = vi.fn(async (_revision: number, preferences: Snapshot["preferences"]) => ({
    ...snapshot,
    revision: 3,
    preferences,
  }));
  render(
    <SettingsPage
      client={{
        load: async () => snapshot,
        save,
        host: testHost({ platform: "windows" }),
        localVoiceModels: fake.client,
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "语音输入" }));

  const card = within(await screen.findByRole("listitem", { name: "快速整句" }));
  fireEvent.click(card.getByRole("button", { name: "使用" }));
  expect(((await screen.findByLabelText("本地模型目录")) as HTMLInputElement).value).toBe(
    sense.path,
  );
  fireEvent.change(screen.getByLabelText("模型下载镜像"), {
    target: { value: "https://ghproxy.example.com" },
  });
  saveSettingsNow();

  await waitFor(() => expect(save).toHaveBeenCalled());
  const saved = save.mock.calls.at(-1)?.[1]?.voice_input;
  expect(saved?.asr_model_path).toBe(sense.path);
  expect(saved?.asr_model_mirror).toBe("https://ghproxy.example.com");
  expect(
    (screen.getByRole("option", { name: "本地模型（离线）" }) as HTMLOptionElement).disabled,
  ).toBe(false);
});
