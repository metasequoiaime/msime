// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type EditionInfo, type FeedbackClient, type Snapshot } from "@msime/ui";
import { feedbackDiagnostics } from "../../../../packages/ui/src/settings/feedback-report-fields";

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
    touch_keyboard_layout: "nine_key",
    global_theme: "paper",
  },
};

const fullEdition: EditionInfo = {
  id: "full",
  input_schemes: ["quanpin", "shuangpin", "wubi"],
  default_scheme: "quanpin",
  temporary_japanese: true,
  neural_keyboard: true,
  offline_glosses: true,
  handwriting: true,
  wubi_mixed_pinyin_default: false,
};

function renderFeedback({
  feedback,
  host = {},
}: {
  feedback?: FeedbackClient;
  host?: Record<string, unknown>;
}) {
  render(
    <SettingsPage
      initialPage="feedback"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        readAppVersion: vi.fn().mockResolvedValue("1.4.2"),
        openExternalUrl: vi.fn(),
        copyText: vi.fn(),
        host: testHost({
          platform: "harmony",
          os_version: "5.0.1",
          edition: fullEdition,
          ...host,
        }),
        home: {
          openSystemKeyboardSettings: vi.fn(),
          setup: {
            read: () => ({ enabled: true, current: false }),
            subscribe: () => () => {},
          },
        },
        feedback,
      }}
    />,
  );
}

async function feedbackPage() {
  return screen.findByRole("group", { name: "帮助与反馈" });
}

function describeField(page: HTMLElement) {
  return within(page).getByRole("textbox", { name: "反馈描述" }) as HTMLTextAreaElement;
}

test("the phone form has the design's type group, description card, counter and an idle submit button", async () => {
  renderFeedback({ feedback: { submit: vi.fn() } });
  const page = await feedbackPage();

  const titles = [...page.querySelectorAll("[data-group-title]")].map((node) => node.textContent);
  expect(titles.slice(0, 2)).toEqual(["类型", "描述"]);
  expect(within(page).getByRole("button", { name: /反馈类型/ }).textContent).toContain("问题");
  const attach = within(page).getByRole("switch", { name: "附带诊断信息" }) as HTMLInputElement;
  expect(attach.checked).toBe(false);
  expect(
    within(page).getByText("包含系统版本、应用版本、当前方案与键盘设置，不含输入内容"),
  ).toBeTruthy();

  const text = describeField(page);
  expect(text.placeholder).toBe("遇到了什么问题？可以写复现步骤、出错的词或期望的结果");
  expect(text.maxLength).toBe(500);
  expect(within(page).getByText("0 / 500")).toBeTruthy();
  const submit = within(page).getByRole("button", { name: "提交" }) as HTMLButtonElement;
  expect(submit.disabled).toBe(true);

  // 只有空白不算一份反馈。
  fireEvent.change(text, { target: { value: "   " } });
  expect(submit.disabled).toBe(true);
  fireEvent.change(text, { target: { value: "候选不对" } });
  expect(within(page).getByText("4 / 500")).toBeTruthy();
  expect(submit.disabled).toBe(false);

  // GitHub 反馈表单是其他宿主的页面；手机上不会两者都绘制。
  expect(within(page).queryByRole("button", { name: "在 GitHub 提交" })).toBeNull();
});

test("submitting sends the chosen type and the trimmed text, then says 已提交 with a toast", async () => {
  const submit = vi.fn().mockResolvedValue(undefined);
  renderFeedback({ feedback: { submit } });
  const page = await feedbackPage();

  const select = page.querySelector("select") as HTMLSelectElement;
  fireEvent.change(select, { target: { value: "dictionary" } });
  fireEvent.change(describeField(page), { target: { value: "  「水杉」打成了「水衫」 " } });
  fireEvent.click(within(page).getByRole("button", { name: "提交" }));

  await waitFor(() =>
    expect(submit).toHaveBeenCalledWith({
      type: "dictionary",
      text: "「水杉」打成了「水衫」",
      diagnostics: null,
    }),
  );
  const done = (await within(page).findByRole("button", { name: "已提交" })) as HTMLButtonElement;
  expect(done.disabled).toBe(true);
  expect(await screen.findByText("已提交，感谢反馈")).toBeTruthy();

  // 之后再编辑文字就是一份新的反馈。
  fireEvent.change(describeField(page), { target: { value: "还有一处" } });
  expect((within(page).getByRole("button", { name: "提交" }) as HTMLButtonElement).disabled).toBe(
    false,
  );
});

test("附带诊断信息 attaches the whitelisted fields and nothing typed", async () => {
  const submit = vi.fn().mockResolvedValue(undefined);
  renderFeedback({ feedback: { submit } });
  const page = await feedbackPage();

  fireEvent.click(within(page).getByRole("switch", { name: "附带诊断信息" }));
  fireEvent.change(describeField(page), { target: { value: "九宫格偶尔不出候选" } });
  fireEvent.click(within(page).getByRole("button", { name: "提交" }));

  await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  expect(submit.mock.calls[0][0].diagnostics).toEqual({
    os: "HarmonyOS 5.0.1",
    app_version: "1.4.2",
    edition: "full",
    scheme: "quanpin",
    keyboard_layout: "nine_key",
    skin: "paper",
    ime_enabled: "true",
    ime_default: "false",
  });
});

test("a refused submission says why under the button and leaves it ready to retry", async () => {
  const submit = vi.fn().mockRejectedValue({ code: "account_rate_limited" });
  renderFeedback({ feedback: { submit } });
  const page = await feedbackPage();

  fireEvent.change(describeField(page), { target: { value: "候选不对" } });
  fireEvent.click(within(page).getByRole("button", { name: "提交" }));

  expect((await within(page).findByRole("alert")).textContent).toBe(
    "提交得太频繁了，请过一会儿再试",
  );
  expect((within(page).getByRole("button", { name: "提交" }) as HTMLButtonElement).disabled).toBe(
    false,
  );
  expect(screen.queryByText("已提交，感谢反馈")).toBeNull();
});

test("a signed-out refusal points at the other channels", async () => {
  const submit = vi.fn().mockRejectedValue({ code: "account_unauthorized" });
  renderFeedback({ feedback: { submit } });
  const page = await feedbackPage();

  fireEvent.change(describeField(page), { target: { value: "候选不对" } });
  fireEvent.click(within(page).getByRole("button", { name: "提交" }));
  expect((await within(page).findByRole("alert")).textContent).toContain("改用下面的渠道");
});

test("帮助 and the community channels stay, below the submit button", async () => {
  renderFeedback({ feedback: { submit: vi.fn() } });
  const page = await feedbackPage();

  const submit = within(page).getByRole("button", { name: "提交" });
  const help = within(page).getByRole("button", { name: "帮助" });
  expect(submit.compareDocumentPosition(help) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  for (const channel of ["GitHub Issues", "QQ 交流群", "Telegram 群组"]) {
    expect(within(page).getByText(channel)).toBeTruthy();
  }
  expect(within(page).getByText("群号：829919142")).toBeTruthy();
});

test("a host without in-app feedback keeps the GitHub report", async () => {
  renderFeedback({});
  const page = await feedbackPage();
  expect(await within(page).findByRole("button", { name: "在 GitHub 提交" })).toBeTruthy();
  expect(within(page).queryByRole("switch", { name: "附带诊断信息" })).toBeNull();
});

test("the 2in1 keeps the GitHub report even when the host can submit", async () => {
  renderFeedback({
    feedback: { submit: vi.fn() },
    host: { mobile_settings: false, panel_windows: true },
  });
  const page = await feedbackPage();
  expect(await within(page).findByRole("button", { name: "在 GitHub 提交" })).toBeTruthy();
  expect(within(page).queryByRole("switch", { name: "附带诊断信息" })).toBeNull();
});

test("diagnostics leave out what the host cannot answer instead of sending it empty", () => {
  expect(
    feedbackDiagnostics({
      appVersion: "1.4.2",
      host: { platform: "harmony" },
      preferences: { scheme: "shuangpin" },
      setup: { enabled: null, current: true },
    }),
  ).toEqual({ app_version: "1.4.2", scheme: "shuangpin", ime_default: "true" });
});
