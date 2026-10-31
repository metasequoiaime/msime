// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  LocalModelManager,
  type LocalVoiceModel,
  type LocalVoiceModelClient,
  type LocalVoiceModelList,
} from "../../../../packages/ui/src/voice/local-models";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function model(title = "合成模型", installed = false): LocalVoiceModel {
  return {
    id: title,
    title,
    description: "合成测试模型",
    languages: ["zh"],
    streaming: false,
    default: true,
    desktop_only: false,
    installed,
    path: `/synthetic/models/${title}`,
    installed_size: 100,
    archive_size: 100,
    memory: 100,
    license_spdx: "MIT",
    license_source: "https://example.com/synthetic-license",
    license_terms: "",
    license_notice: "合成许可说明",
    hotwords: "native",
    import_files: [],
  };
}

function listing(models: LocalVoiceModel[]): LocalVoiceModelList {
  return { models, default: models[0].id, root: "/synthetic/models" };
}

function service(title = "合成模型", installed = false) {
  return {
    list: vi.fn(async () => listing([model(title, installed)])),
    install: vi.fn(async (_id: string) => "/synthetic/installed"),
    import: vi.fn(async (_id: string): Promise<string | null> => "/synthetic/imported"),
    cancel: vi.fn(async () => true),
    remove: vi.fn(async (_id: string) => {}),
    onProgress: vi.fn(async () => () => {}),
  } satisfies LocalVoiceModelClient;
}

function props() {
  return {
    mobile: false,
    modelPath: "",
    onUse: vi.fn(),
    onRemoved: vi.fn(),
    confirm: vi.fn(async () => true),
  };
}

test.each(["success", "failure"])("客户端复用后忽略旧列表的 %s", async (outcome) => {
  const first = service("当前模型");
  const second = service("另一个模型");
  const old = deferred<LocalVoiceModelList>();
  first.list.mockReturnValueOnce(old.promise);
  const shared = props();
  const view = render(<LocalModelManager {...shared} client={first} />);
  view.rerender(<LocalModelManager {...shared} client={second} />);
  view.rerender(<LocalModelManager {...shared} client={first} />);
  await screen.findByRole("listitem", { name: "当前模型" });

  await act(async () => {
    if (outcome === "success") old.resolve(listing([model("过期模型")]));
    else old.reject({ code: "local_model_network" });
  });
  expect(screen.getByRole("listitem", { name: "当前模型" })).toBeTruthy();
  expect(screen.queryByRole("listitem", { name: "过期模型" })).toBeNull();
  expect(screen.queryByRole("status")).toBeNull();
});

test.each([
  ["install", "success"],
  ["install", "failure"],
  ["import", "success"],
  ["import", "failure"],
] as const)("客户端复用后旧 %s 的 %s 不改变新操作", async (operation, outcome) => {
  const first = service();
  const second = service("另一个模型");
  const old = deferred<string>();
  const next = deferred<string>();
  first[operation].mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
  const shared = props();
  const view = render(<LocalModelManager {...shared} client={first} />);
  const label = operation === "install" ? /下载（/ : "从文件导入";
  const cancelLabel = operation === "install" ? "取消下载" : "取消导入";
  fireEvent.click(
    within(await screen.findByRole("listitem", { name: "合成模型" })).getByRole("button", {
      name: label,
    }),
  );

  view.rerender(<LocalModelManager {...shared} client={second} />);
  view.rerender(<LocalModelManager {...shared} client={first} />);
  const card = within(await screen.findByRole("listitem", { name: "合成模型" }));
  fireEvent.click(card.getByRole("button", { name: label }));
  await act(async () => {
    if (outcome === "success") old.resolve("/synthetic/old-operation");
    else old.reject({ code: "local_model_checksum_mismatch" });
  });
  expect(shared.onUse).not.toHaveBeenCalled();
  expect(card.getByRole("button", { name: cancelLabel })).toBeTruthy();
  expect(screen.queryByRole("status")).toBeNull();
  expect(first.list).toHaveBeenCalledTimes(2);

  await act(async () => next.resolve("/synthetic/current-operation"));
  expect(shared.onUse).toHaveBeenCalledExactlyOnceWith("/synthetic/current-operation");
});

test.each(["switch", "reuse", "unmount"])("删除确认等待期间 %s 后不再删除", async (change) => {
  const first = service("合成模型", true);
  const second = service("另一个模型", true);
  const confirmation = deferred<boolean>();
  const shared = props();
  shared.confirm.mockReturnValueOnce(confirmation.promise);
  const view = render(<LocalModelManager {...shared} client={first} />);
  fireEvent.click(
    within(await screen.findByRole("listitem", { name: "合成模型" })).getByRole("button", {
      name: "删除",
    }),
  );
  expect(shared.confirm).toHaveBeenCalledOnce();
  if (change === "unmount") view.unmount();
  else {
    view.rerender(<LocalModelManager {...shared} client={second} />);
    if (change === "reuse") view.rerender(<LocalModelManager {...shared} client={first} />);
  }
  await act(async () => confirmation.resolve(true));
  expect(first.remove).not.toHaveBeenCalled();
  expect(second.remove).not.toHaveBeenCalled();
  expect(shared.onRemoved).not.toHaveBeenCalled();
});

test.each(["success", "failure"])("客户端复用后忽略已发出删除的 %s", async (outcome) => {
  const first = service("合成模型", true);
  const second = service("另一个模型", true);
  const removal = deferred<void>();
  first.remove.mockReturnValueOnce(removal.promise);
  const shared = props();
  const view = render(<LocalModelManager {...shared} client={first} />);
  fireEvent.click(
    within(await screen.findByRole("listitem", { name: "合成模型" })).getByRole("button", {
      name: "删除",
    }),
  );
  await waitFor(() => expect(first.remove).toHaveBeenCalledOnce());
  view.rerender(<LocalModelManager {...shared} client={second} />);
  view.rerender(<LocalModelManager {...shared} client={first} />);
  await screen.findByRole("listitem", { name: "合成模型" });
  await act(async () => {
    if (outcome === "success") removal.resolve();
    else removal.reject({ code: "local_model_io" });
  });
  expect(shared.onRemoved).not.toHaveBeenCalled();
  expect(screen.queryByRole("status")).toBeNull();
  expect(first.list).toHaveBeenCalledTimes(2);
});

test.each(["success", "failure"])("并行安装后迟到的旧刷新 %s 不能覆盖最新状态", async (outcome) => {
  const client = service();
  const first = model("合成甲");
  const second = model("合成乙");
  const stale = deferred<LocalVoiceModelList>();
  client.list
    .mockResolvedValueOnce(listing([first, second]))
    .mockReturnValueOnce(stale.promise)
    .mockResolvedValue(
      listing([
        { ...first, installed: true },
        { ...second, installed: true },
      ]),
    );
  const installFirst = deferred<string>();
  const installSecond = deferred<string>();
  client.install
    .mockReturnValueOnce(installFirst.promise)
    .mockReturnValueOnce(installSecond.promise);
  render(<LocalModelManager {...props()} client={client} modelPath="/synthetic/selected" />);
  fireEvent.click(
    within(await screen.findByRole("listitem", { name: "合成甲" })).getByRole("button", {
      name: /下载（/,
    }),
  );
  fireEvent.click(
    within(screen.getByRole("listitem", { name: "合成乙" })).getByRole("button", {
      name: /下载（/,
    }),
  );
  await act(async () => installFirst.resolve(first.path));
  expect(client.list).toHaveBeenCalledTimes(2);
  await act(async () => installSecond.resolve(second.path));
  expect(within(screen.getByRole("listitem", { name: "合成乙" })).getByText("已下载")).toBeTruthy();
  await act(async () => {
    if (outcome === "success") stale.resolve(listing([{ ...first, installed: true }, second]));
    else stale.reject({ code: "local_model_io" });
  });
  expect(within(screen.getByRole("listitem", { name: "合成乙" })).getByText("已下载")).toBeTruthy();
  expect(screen.getByRole("status").textContent).toBe("「合成乙」已下载。");
});
