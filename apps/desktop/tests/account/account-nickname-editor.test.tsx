// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountNicknameEditor } from "../../../../packages/ui/src/account/account-nickname-editor";

afterEach(cleanup);

test("mobile nickname editor shows validation guidance and a codepoint count", () => {
  const onChange = vi.fn();
  render(
    <AccountNicknameEditor
      variant="mobile"
      value={"invalid\nname"}
      normalizedName={"invalid\nname"}
      validName={false}
      maximumChars={64}
      maximumUtf16Units={128}
      checking={false}
      unavailable={false}
      disabled={false}
      onChange={onChange}
    />,
  );
  expect(screen.getByText("昵称最多 64 个字符，请勿使用换行或控制字符。")).not.toBeNull();
  expect(screen.getByText("12/64")).not.toBeNull();
  fireEvent.change(screen.getByRole("textbox", { name: "编辑社区昵称" }), {
    target: { value: "Next User" },
  });
  expect(onChange).toHaveBeenCalledWith("Next User");
});

test("sheet and desktop nickname editors keep their distinct presentation", () => {
  const props = {
    value: "合成👋",
    normalizedName: "合成👋",
    validName: true,
    maximumChars: 64,
    maximumUtf16Units: 128,
    checking: false,
    unavailable: false,
    disabled: false,
    onChange: vi.fn(),
  };
  const { rerender } = render(<AccountNicknameEditor variant="sheet" {...props} />);
  expect(screen.getByText("3/64")).not.toBeNull();
  expect(screen.getByText("社区昵称").className).toContain("sr-only");
  expect(screen.getByRole("textbox", { name: "编辑社区昵称" }).className).toContain("h-[50px]");

  rerender(<AccountNicknameEditor variant="desktop" {...props} />);
  expect(screen.getByText("昵称会显示在社区作品中，已发布的作品也会同步更新。")).not.toBeNull();
  expect(screen.queryByText("3/64")).toBeNull();
});

test.each(["mobile", "sheet", "desktop"] as const)(
  "%s 昵称框使用共享长度上限且检查中不显示错误样式",
  (variant) => {
    const { rerender } = render(
      <AccountNicknameEditor
        variant={variant}
        value="合成🌲"
        normalizedName="合成🌲"
        validName={false}
        disabled={false}
        maximumChars={12}
        maximumUtf16Units={24}
        checking={true}
        unavailable={false}
        onChange={vi.fn()}
      />,
    );
    expect(screen.getByRole("textbox", { name: "编辑社区昵称" })).toHaveProperty("maxLength", 24);
    expect(screen.getByText("正在检查昵称…").className).not.toContain("error");
    rerender(
      <AccountNicknameEditor
        variant={variant}
        value="合成🌲"
        normalizedName="合成🌲"
        validName={true}
        disabled={false}
        maximumChars={12}
        maximumUtf16Units={24}
        checking={false}
        unavailable={false}
        onChange={vi.fn()}
      />,
    );
    if (variant !== "desktop") expect(screen.getByText("3/12")).not.toBeNull();
  },
);

test.each(["mobile", "sheet", "desktop"] as const)(
  "%s 显示共享动态长度对应的无效提示",
  (variant) => {
    render(
      <AccountNicknameEditor
        variant={variant}
        value="合成无效昵称"
        normalizedName="合成无效昵称"
        validName={false}
        disabled={false}
        maximumChars={12}
        maximumUtf16Units={24}
        checking={false}
        unavailable={false}
        onChange={vi.fn()}
      />,
    );
    expect(screen.getByText("昵称最多 12 个字符，请勿使用换行或控制字符。")).not.toBeNull();
  },
);
