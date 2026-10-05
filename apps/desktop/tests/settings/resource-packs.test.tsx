// @vitest-environment jsdom
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  HandwritingPanel,
  SettingsPage,
  type LocalVoiceModelProgress,
  type ResourcePackClient,
  type ResourcePackId,
  type ResourcePackStatus,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const initial: Snapshot = {
  format_version: 1,
  revision: 7,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

const sizes: Record<ResourcePackId, number> = {
  japanese: 66_544_207,
  "language-dictionaries": 18_900_000,
  handwriting: 26_861_246,
  "settled-model": 25_480_184,
};

/** 一个假的资源包服务：install 返回的 promise 由测试决定何时完成或失败，进度由测试手动推送。`offered` 是宿主列出的资源包，缺省是 macOS 列出的全部。 */
function fakePacks(
  state: Partial<Record<ResourcePackId, ResourcePackStatus["state"]>> = {},
  offered: ResourcePackId[] = ["japanese", "language-dictionaries", "handwriting", "settled-model"],
) {
  const states: Record<ResourcePackId, ResourcePackStatus["state"]> = {
    japanese: "missing",
    "language-dictionaries": "missing",
    handwriting: "missing",
    "settled-model": "missing",
    ...state,
  };
  const listeners = new Set<(progress: LocalVoiceModelProgress) => void>();
  const pending = new Map<
    ResourcePackId,
    { resolve: (path: string) => void; reject: (error: unknown) => void }
  >();
  const client = {
    list: vi.fn(async () =>
      offered.map((id) => ({
        id,
        state: states[id],
        size: sizes[id],
        schemes: [],
      })),
    ),
    install: vi.fn(
      (id: ResourcePackId) =>
        new Promise<string>((resolve, reject) => pending.set(id, { resolve, reject })),
    ),
    cancel: vi.fn(async () => true),
    onProgress: vi.fn(async (listener: (progress: LocalVoiceModelProgress) => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    }),
  } satisfies ResourcePackClient;
  return {
    client,
    emit(progress: LocalVoiceModelProgress) {
      act(() => listeners.forEach((listener) => listener(progress)));
    },
    async finish(id: ResourcePackId) {
      states[id] = "installed";
      await act(async () => pending.get(id)?.resolve(`/state/resource-packs/${id}`));
    },
    async fail(id: ResourcePackId, code: string) {
      await act(async () => pending.get(id)?.reject({ code }));
    },
  };
}

function renderSettings(platform: string, resourcePacks?: ResourcePackClient, snapshot = initial) {
  const save = vi.fn().mockImplementation(async (_revision, preferences) => ({
    ...snapshot,
    revision: snapshot.revision + 1,
    preferences,
  }));
  render(
    <SettingsPage
      initialPage="input"
      client={{
        load: vi.fn().mockResolvedValue(snapshot),
        save,
        host: testHost({
          platform,
          input_schemes: [
            "quanpin",
            "shuangpin",
            "wubi",
            "japanese",
            "korean",
            "cantonese",
            "stroke",
          ],
        }),
        resourcePacks,
      }}
    />,
  );
  return save;
}

function schemeGroup() {
  return screen.getByRole("region", { name: "方案" });
}

test("picking 日文 saves the scheme at once and downloads the Japanese dictionary", async () => {
  const packs = fakePacks();
  const save = renderSettings("macos", packs.client);
  await settingsFormReady();
  await waitFor(() => expect(packs.client.list).toHaveBeenCalled());
  // 列表读到之后才会下载；先等列表里的状态进入页面。
  await screen.findByRole("switch", { name: /^临时日语/ });
  await screen.findByRole("button", { name: "下载日文词库" });

  fireEvent.click(within(schemeGroup()).getByRole("radio", { name: "日文" }));
  expect(packs.client.install).toHaveBeenCalledTimes(1);
  expect(packs.client.install).toHaveBeenCalledWith("japanese");
  // 下载还在进行，方案已经保存，运行时在词库到位前自行回退。
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({ scheme: "japanese", last_chinese_scheme: "quanpin" }),
    ),
  );
  expect(packs.client.install).toHaveBeenCalledTimes(1);
});

test("picking 粤拼 downloads the language dictionaries and shows the progress", async () => {
  const packs = fakePacks();
  renderSettings("macos", packs.client);
  await settingsFormReady();
  await waitFor(() => expect(packs.client.list).toHaveBeenCalled());
  await screen.findByRole("button", { name: "下载日文词库" });

  fireEvent.click(within(schemeGroup()).getByRole("radio", { name: "粤拼" }));
  expect(packs.client.install).toHaveBeenCalledWith("language-dictionaries");
  packs.emit({
    id: "language-dictionaries",
    stage: "download",
    downloaded: 9_450_000,
    total: 18_900_000,
  });
  expect(await screen.findByText("下载中 50%")).toBeTruthy();
  expect(screen.getByRole("button", { name: "取消下载粤语、注音与笔画词库" })).toBeTruthy();

  await packs.finish("language-dictionaries");
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: "取消下载粤语、注音与笔画词库" })).toBeNull(),
  );
  // 装好后重新读列表，行随之消失。
  expect(screen.queryByRole("button", { name: "下载粤语、注音与笔画词库" })).toBeNull();
});

test("picking 笔画 downloads the same language dictionaries pack", async () => {
  const packs = fakePacks();
  const save = renderSettings("macos", packs.client);
  await settingsFormReady();
  await waitFor(() => expect(packs.client.list).toHaveBeenCalled());

  fireEvent.click(within(schemeGroup()).getByRole("radio", { name: "笔画" }));
  expect(packs.client.install).toHaveBeenCalledWith("language-dictionaries");
  expect(await screen.findByRole("button", { name: "取消下载粤语、注音与笔画词库" })).toBeTruthy();
  await saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({ scheme: "stroke", last_chinese_scheme: "stroke" }),
    ),
  );
});

test("a network failure shows the reason and a retry, and nothing retries by itself", async () => {
  const packs = fakePacks();
  renderSettings("macos", packs.client, {
    ...initial,
    preferences: { ...initial.preferences, scheme: "japanese", last_chinese_scheme: "quanpin" },
  });
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("button", { name: "下载日文词库" }));
  expect(packs.client.install).toHaveBeenCalledTimes(1);

  await packs.fail("japanese", "local_model_network");
  expect(await screen.findByText(/下载失败：无法连接下载服务器/)).toBeTruthy();
  const retry = screen.getByRole("button", { name: "重新下载日文词库" });
  // 等几轮渲染，确认失败之后不会自己再装一次。
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
  expect(packs.client.install).toHaveBeenCalledTimes(1);

  fireEvent.click(retry);
  await waitFor(() => expect(packs.client.install).toHaveBeenCalledTimes(2));
  expect(screen.queryByText(/下载失败：无法连接下载服务器/)).toBeNull();
});

test("a cancelled download clears the error and offers the download again", async () => {
  const packs = fakePacks();
  renderSettings("macos", packs.client, {
    ...initial,
    preferences: { ...initial.preferences, scheme: "japanese", last_chinese_scheme: "quanpin" },
  });
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("button", { name: "下载日文词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "取消下载日文词库" }));
  expect(packs.client.cancel).toHaveBeenCalledWith("japanese");
  await packs.fail("japanese", "local_model_cancelled");
  expect(await screen.findByRole("button", { name: "下载日文词库" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "重新下载日文词库" })).toBeNull();
});

test("a host that bundles the dictionaries never downloads them", async () => {
  // Windows 和 Linux 随包带着日文词典和语言词库，宿主的列表里没有它们。
  const packs = fakePacks({}, ["settled-model"]);
  renderSettings("windows", packs.client);
  await settingsFormReady();
  await waitFor(() => expect(packs.client.list).toHaveBeenCalled());
  fireEvent.click(within(schemeGroup()).getByRole("radio", { name: "粤拼" }));
  fireEvent.click(within(schemeGroup()).getByRole("radio", { name: "日文" }));
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
  expect(packs.client.install).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "下载日文词库" })).toBeNull();
  expect(screen.queryByText("临时日语词库")).toBeNull();
});

test("a host without a resource pack service never asks for one", async () => {
  renderSettings("windows");
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("switch", { name: "桌面神经联想" }));
  expect(screen.queryByRole("button", { name: "下载桌面神经联想模型" })).toBeNull();
});

test("turning on 桌面神经联想 downloads its model and shows the progress", async () => {
  const packs = fakePacks({}, ["handwriting", "settled-model"]);
  const save = renderSettings("windows", packs.client);
  await settingsFormReady();
  const neural = await screen.findByRole("switch", { name: "桌面神经联想" });
  await waitFor(() => expect(screen.getByText(/需下载约 25 MB 模型/)).toBeTruthy());
  // 开关关着时不下载，也不列出下载行。
  expect(packs.client.install).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "下载桌面神经联想模型" })).toBeNull();

  fireEvent.click(neural);
  expect(packs.client.install).toHaveBeenCalledTimes(1);
  expect(packs.client.install).toHaveBeenCalledWith("settled-model");
  packs.emit({ id: "settled-model", stage: "download", downloaded: 12_740_092, total: 25_480_184 });
  expect(await screen.findByText("下载中 50%")).toBeTruthy();
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        sentence_association: expect.objectContaining({ neural_desktop: true }),
      }),
    ),
  );

  await packs.finish("settled-model");
  await waitFor(() =>
    expect(screen.queryByRole("button", { name: "取消下载桌面神经联想模型" })).toBeNull(),
  );
  expect(screen.queryByText(/需下载约/)).toBeNull();
});

test("a bundled 桌面神经联想 model needs no download", async () => {
  // 随包带着落定重排模型时，宿主不列出它。
  const packs = fakePacks({}, ["handwriting"]);
  renderSettings("linux", packs.client);
  await settingsFormReady();
  await waitFor(() => expect(packs.client.list).toHaveBeenCalled());
  fireEvent.click(await screen.findByRole("switch", { name: "桌面神经联想" }));
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
  expect(packs.client.install).not.toHaveBeenCalled();
  expect(screen.queryByText(/需下载约/)).toBeNull();
});

test("a failed download offers the download mirror in place", async () => {
  const packs = fakePacks({}, ["settled-model"]);
  const save = renderSettings("windows", packs.client, {
    ...initial,
    preferences: { ...initial.preferences, sentence_association: { neural_desktop: true } },
  });
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("button", { name: "下载桌面神经联想模型" }));
  await packs.fail("settled-model", "local_model_network");
  expect(await screen.findByText(/请检查网络，或设置下载镜像后再试/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "重新下载桌面神经联想模型" })).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "设置下载镜像" }));
  fireEvent.change(screen.getByRole("textbox", { name: "模型下载镜像" }), {
    target: { value: "https://mirror.example.com" },
  });
  saveSettingsNow();
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      7,
      expect.objectContaining({
        voice_input: expect.objectContaining({ asr_model_mirror: "https://mirror.example.com" }),
      }),
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "重新下载桌面神经联想模型" }));
  await waitFor(() => expect(packs.client.install).toHaveBeenCalledTimes(2));
});

test("retrying right after typing a mirror saves the mirror before downloading", async () => {
  const packs = fakePacks({}, ["settled-model"]);
  const save = renderSettings("windows", packs.client, {
    ...initial,
    preferences: { ...initial.preferences, sentence_association: { neural_desktop: true } },
  });
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("button", { name: "下载桌面神经联想模型" }));
  await packs.fail("settled-model", "local_model_network");
  fireEvent.click(await screen.findByRole("button", { name: "设置下载镜像" }));
  fireEvent.change(screen.getByRole("textbox", { name: "模型下载镜像" }), {
    target: { value: "https://mirror.example.com" },
  });
  // 不等自动保存的倒计时，直接点重试：宿主按已保存的偏好取镜像，所以镜像必须先写进去。
  expect(save).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "重新下载桌面神经联想模型" }));
  await waitFor(() => expect(packs.client.install).toHaveBeenCalledTimes(2));
  const saved = save.mock.calls.findIndex(
    ([, preferences]) => preferences.voice_input?.asr_model_mirror === "https://mirror.example.com",
  );
  expect(saved).toBeGreaterThanOrEqual(0);
  expect(save.mock.invocationCallOrder[saved]).toBeLessThan(
    packs.client.install.mock.invocationCallOrder[1],
  );
});

test("临时日语 offers the Japanese dictionary but never downloads it on its own", async () => {
  const packs = fakePacks();
  renderSettings("macos", packs.client);
  await settingsFormReady();
  const download = await screen.findByRole("button", { name: "下载日文词库" });
  expect(screen.getByText(/临时日语需要它/)).toBeTruthy();
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
  expect(packs.client.install).not.toHaveBeenCalled();

  fireEvent.click(download);
  expect(packs.client.install).toHaveBeenCalledWith("japanese");
});

test("an installed Japanese dictionary leaves no row behind", async () => {
  const packs = fakePacks({ japanese: "installed" });
  renderSettings("macos", packs.client);
  await settingsFormReady();
  await waitFor(() => expect(packs.client.list).toHaveBeenCalled());
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
  expect(screen.queryByRole("button", { name: "下载日文词库" })).toBeNull();
  expect(screen.queryByText("临时日语词库")).toBeNull();
});

test("the macOS handwriting panel downloads its model once and recognises the waiting ink after", async () => {
  const packs = fakePacks();
  const recognizeHandwriting = vi.fn().mockResolvedValue({ candidates: ["水"] });
  render(
    <HandwritingPanel
      platform="macos"
      client={{
        close: vi.fn().mockResolvedValue(undefined),
        recognizeHandwriting,
        copyHandwritingCandidate: vi.fn().mockResolvedValue(undefined),
        resourcePacks: packs.client,
      }}
    />,
  );
  await waitFor(() => expect(packs.client.install).toHaveBeenCalledWith("handwriting"));
  packs.emit({ id: "handwriting", stage: "download", downloaded: 13_430_623, total: 26_861_246 });
  expect(await screen.findByText(/正在下载手写模型（约 27 MB）… 50%/)).toBeTruthy();

  const canvas = screen.getByLabelText("手写画布");
  fireEvent.pointerDown(canvas, { pointerId: 1, clientX: 10, clientY: 10, isPrimary: true });
  fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 40, clientY: 40 });
  fireEvent.pointerUp(canvas, { pointerId: 1, clientX: 60, clientY: 60 });
  expect(await screen.findByText("手写模型下载完成后自动识别")).toBeTruthy();
  expect(recognizeHandwriting).not.toHaveBeenCalled();

  await packs.finish("handwriting");
  await waitFor(() => expect(recognizeHandwriting).toHaveBeenCalledTimes(1));
  expect(await screen.findByRole("button", { name: /水/ })).toBeTruthy();
  expect(packs.client.install).toHaveBeenCalledTimes(1);
});

test("a handwriting download failure offers a retry instead of looping", async () => {
  const packs = fakePacks();
  render(
    <HandwritingPanel
      platform="macos"
      client={{
        close: vi.fn().mockResolvedValue(undefined),
        recognizeHandwriting: vi.fn().mockResolvedValue({ candidates: [] }),
        resourcePacks: packs.client,
      }}
    />,
  );
  await waitFor(() => expect(packs.client.install).toHaveBeenCalledTimes(1));
  await packs.fail("handwriting", "local_model_network");
  const retry = await screen.findByRole("button", { name: "重试" });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
  expect(packs.client.install).toHaveBeenCalledTimes(1);
  fireEvent.click(retry);
  expect(packs.client.install).toHaveBeenCalledTimes(2);
});

test("a handwriting panel whose host does not list the model recognises at once", async () => {
  // Windows 上 Ink 有中文识别器（或者随包带着模型）时宿主不列出手写模型：不下载，也不让识别等下载。
  const packs = fakePacks({}, ["settled-model"]);
  const recognizeHandwriting = vi.fn().mockResolvedValue({ candidates: ["水"] });
  render(
    <HandwritingPanel
      platform="windows"
      client={{
        close: vi.fn().mockResolvedValue(undefined),
        recognizeHandwriting,
        copyHandwritingCandidate: vi.fn().mockResolvedValue(undefined),
        resourcePacks: packs.client,
      }}
    />,
  );
  await waitFor(() => expect(packs.client.list).toHaveBeenCalled());
  const canvas = screen.getByLabelText("手写画布");
  fireEvent.pointerDown(canvas, { pointerId: 1, clientX: 10, clientY: 10, isPrimary: true });
  fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 40, clientY: 40 });
  fireEvent.pointerUp(canvas, { pointerId: 1, clientX: 60, clientY: 60 });
  await waitFor(() => expect(recognizeHandwriting).toHaveBeenCalledTimes(1));
  expect(await screen.findByRole("button", { name: /水/ })).toBeTruthy();
  expect(packs.client.install).not.toHaveBeenCalled();
  expect(screen.queryByText(/手写模型/)).toBeNull();
});

test("the Windows handwriting panel downloads the model when the host lists it", async () => {
  const packs = fakePacks({}, ["handwriting"]);
  render(
    <HandwritingPanel
      platform="windows"
      client={{
        close: vi.fn().mockResolvedValue(undefined),
        recognizeHandwriting: vi.fn().mockResolvedValue({ candidates: [] }),
        resourcePacks: packs.client,
      }}
    />,
  );
  await waitFor(() => expect(packs.client.install).toHaveBeenCalledWith("handwriting"));
});

test("a handwriting download failure offers the download mirror and retries after saving it", async () => {
  const packs = fakePacks();
  const modelMirror = {
    load: vi.fn().mockResolvedValue(""),
    save: vi.fn().mockResolvedValue(undefined),
  };
  render(
    <HandwritingPanel
      platform="linux"
      client={{
        close: vi.fn().mockResolvedValue(undefined),
        recognizeHandwriting: vi.fn().mockResolvedValue({ candidates: [] }),
        resourcePacks: packs.client,
        modelMirror,
      }}
    />,
  );
  await waitFor(() => expect(packs.client.install).toHaveBeenCalledTimes(1));
  await packs.fail("handwriting", "local_model_network");
  fireEvent.click(await screen.findByRole("button", { name: "设置下载镜像" }));
  const input = await screen.findByRole("textbox", { name: "模型下载镜像" });
  expect(modelMirror.load).toHaveBeenCalledTimes(1);

  fireEvent.change(input, { target: { value: "http://insecure.example.com" } });
  fireEvent.click(screen.getByRole("button", { name: "保存并重试" }));
  expect(await screen.findByText("下载镜像地址无效，必须以 https:// 开头。")).toBeTruthy();
  expect(modelMirror.save).not.toHaveBeenCalled();

  fireEvent.change(input, { target: { value: " https://mirror.example.com " } });
  fireEvent.click(screen.getByRole("button", { name: "保存并重试" }));
  await waitFor(() => expect(modelMirror.save).toHaveBeenCalledWith("https://mirror.example.com"));
  await waitFor(() => expect(packs.client.install).toHaveBeenCalledTimes(2));
});
