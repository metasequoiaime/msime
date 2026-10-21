// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { HelpSettingsPage, type EditionInfo } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const common = {
  busy: false,
  hidden: false,
  mobile: false,
  ios: false,
  android: false,
  macos: false,
  platformHelpIntro: "平台简介",
  platformQuickStart: "快速开始",
  platformNetworkDescription: "网络说明",
};

test("renders macOS guide cards and opens documentation", () => {
  const onOpenDocumentation = vi.fn();
  render(<HelpSettingsPage {...common} macos onOpenDocumentation={onOpenDocumentation} />);

  expect(screen.getByRole("group", { name: "开始输入" })).toBeTruthy();
  expect(screen.getByText("候选词释义")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "打开官网" }));
  expect(onOpenDocumentation).toHaveBeenCalledOnce();
});

test("renders mobile help and opens system keyboard settings", () => {
  const onOpenSystemKeyboardSettings = vi.fn();
  render(
    <HelpSettingsPage
      {...common}
      mobile
      ios
      onOpenSystemKeyboardSettings={onOpenSystemKeyboardSettings}
    />,
  );

  expect(screen.getByText("平台简介")).toBeTruthy();
  expect(screen.getByText("允许完全访问")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "打开系统键盘设置" }));
  expect(onOpenSystemKeyboardSettings).toHaveBeenCalledOnce();
});

test("marks the mobile system settings action busy", () => {
  const onOpenSystemKeyboardSettings = vi.fn();
  render(
    <HelpSettingsPage
      {...common}
      busy
      mobile
      ios
      onOpenSystemKeyboardSettings={onOpenSystemKeyboardSettings}
    />,
  );

  const button = screen.getByRole("button", { name: "打开系统键盘设置" });
  expect(button.getAttribute("aria-busy")).toBe("true");
  expect((button as HTMLButtonElement).disabled).toBe(true);
});

test("hides the page when requested", () => {
  const { container } = render(<HelpSettingsPage {...common} hidden />);
  expect(container.querySelector("fieldset")?.getAttribute("hidden")).not.toBeNull();
});

test("Linux 也用术语卡片，键名和排查步骤是 Linux 的，并提供打开官网", () => {
  const onOpenDocumentation = vi.fn();
  render(<HelpSettingsPage {...common} linux onOpenDocumentation={onOpenDocumentation} />);

  const start = screen.getByRole("group", { name: "开始输入" });
  expect(start.textContent).toContain("Ctrl+Shift+F");
  expect(start.textContent).not.toContain("Option");
  const problems = screen.getByRole("group", { name: "遇到问题" });
  expect(problems.textContent).toContain("fcitx5-configtool");
  expect(problems.textContent).toContain("ibus restart");
  expect(problems.textContent).toContain("Ctrl+Shift+Alt+R");
  // 自己下载的词库不随软件包更新，排查卡片要指向「立即更新」和终端命令，与词库过期的通知一致。
  expect(problems.textContent).toContain("立即更新");
  expect(problems.textContent).toContain("msime-linux-setup --update --download");
  // 云候选、语音和 AI 走用户级服务、凭据存在本机文件里，这段联网说明留在 Linux 的帮助页上。
  expect(screen.getByText("网络说明")).toBeTruthy();
  // 散文版的「快速上手」不再出现，macOS 那一行「打开官网」也给 Linux。
  expect(screen.queryByText("快速开始")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "打开官网" }));
  expect(onOpenDocumentation).toHaveBeenCalledOnce();
});

/** 合成的非 full 版本：名字和方案取自版本表的形状，id 不对应任何真实安装。 */
function edition(
  id: string,
  displayName: string,
  schemes: EditionInfo["input_schemes"],
): EditionInfo {
  return {
    id,
    display_name: displayName,
    input_schemes: schemes,
    default_scheme: schemes[0],
    temporary_japanese: false,
    neural_keyboard: false,
    offline_glosses: false,
    handwriting: false,
    wubi_mixed_pinyin_default: false,
  };
}

test("Linux 帮助卡片里的输入法条目名和命令随安装的版本，不写死 full 的", () => {
  render(<HelpSettingsPage {...common} linux edition={edition("wubi", "水杉五笔", ["wubi"])} />);

  const problems = screen.getByRole("group", { name: "遇到问题" });
  expect(problems.textContent).toContain("「水杉五笔」");
  expect(problems.textContent).toContain("「Metasequoia 水杉五笔」");
  expect(problems.textContent).toContain("msime-linux-wubi-setup --register");
  expect(problems.textContent).toContain("msime-linux-wubi-setup --update --download");
  expect(problems.textContent).not.toContain("水杉输入法");
  expect(problems.textContent).not.toContain("msime-linux-setup");
  // 五笔是中文方案，简繁和中文标点照样列出。
  expect(screen.getByRole("group", { name: "开始输入" }).textContent).toContain("Ctrl+Shift+F");
});

test("不提供中文方案的 Linux 版本不列简繁和中文标点", () => {
  render(
    <HelpSettingsPage {...common} linux edition={edition("japanese", "水杉日语", ["japanese"])} />,
  );

  const start = screen.getByRole("group", { name: "开始输入" });
  expect(start.textContent).not.toContain("Ctrl+Shift+F");
  expect(start.textContent).not.toContain("Ctrl+.");
  expect(screen.getByRole("group", { name: "遇到问题" }).textContent).toContain(
    "msime-linux-japanese-setup",
  );
});
