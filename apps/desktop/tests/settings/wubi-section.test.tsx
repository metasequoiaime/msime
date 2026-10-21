// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { WubiSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("Wubi section updates the mixed-pinyin preference", () => {
  const onChange = vi.fn();
  render(
    <WubiSection
      preferences={{ wubi_mixed_pinyin: false, wubi_code_hint: true }}
      onChange={onChange}
    />,
  );

  // 引擎打开混输后每次都查全拼，拼音候选接在五笔候选之后，不只是在五笔码表答不上时才出现，说明要照这个写。
  expect(
    screen.getByText("五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选。"),
  ).toBeTruthy();
  fireEvent.click(screen.getByLabelText("五笔拼音混输"));
  expect(onChange).toHaveBeenCalledWith({ wubi_mixed_pinyin: true });
});

test("Wubi section renders the enabled defaults", () => {
  const onChange = vi.fn();
  render(<WubiSection preferences={{}} onChange={onChange} />);

  expect((screen.getByLabelText("五笔拼音混输") as HTMLInputElement).checked).toBe(false);
  expect((screen.getByLabelText("候选显示剩余编码") as HTMLInputElement).checked).toBe(true);
  // 没有开关的那段时间里四码唯一总是自动上屏，所以文档里没有它时开关显示为开。
  expect((screen.getByLabelText("五笔四码唯一候选自动上屏") as HTMLInputElement).checked).toBe(
    true,
  );
  // z 通配是来源出厂关着的那一项；顶字则一直常开，缺键按开显示。
  expect((screen.getByLabelText("z 键通配") as HTMLInputElement).checked).toBe(false);
  expect((screen.getByLabelText("第五码将首选上屏") as HTMLInputElement).checked).toBe(true);
});

test("五笔 z 通配与第五码顶字各写各的共享偏好", () => {
  const onChange = vi.fn();
  render(
    <WubiSection
      preferences={{ wubi_z_wildcard: false, wubi_fifth_code_top_commit: true }}
      onChange={onChange}
    />,
  );

  fireEvent.click(screen.getByLabelText("z 键通配"));
  expect(onChange).toHaveBeenLastCalledWith({ wubi_z_wildcard: true });
  fireEvent.click(screen.getByLabelText("第五码将首选上屏"));
  expect(onChange).toHaveBeenLastCalledWith({ wubi_fifth_code_top_commit: false });
});

test("Wubi section writes the auto-commit preference through the shared patch", () => {
  const onChange = vi.fn();
  render(<WubiSection preferences={{ wubi_auto_commit_unique: true }} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("五笔四码唯一候选自动上屏"));
  expect(onChange).toHaveBeenCalledWith({ wubi_auto_commit_unique: false });
});

test("五笔快捷选词默认关闭，启用后可一起修改二三候选键", () => {
  const onChange = vi.fn();
  const { rerender } = render(<WubiSection preferences={{}} onChange={onChange} />);
  const toggle = screen.getByLabelText("五笔快捷选词") as HTMLInputElement;
  expect(toggle.checked).toBe(false);
  expect(screen.queryByLabelText("五笔第 2 候选键")).toBeNull();
  fireEvent.click(toggle);
  expect(onChange).toHaveBeenLastCalledWith({
    wubi_candidate_selection: { enabled: true, second_key: ";", third_key: "'" },
  });
  rerender(
    <WubiSection
      preferences={{ wubi_candidate_selection: { enabled: true, second_key: ";", third_key: "'" } }}
      onChange={onChange}
    />,
  );
  onChange.mockClear();
  fireEvent.change(screen.getByLabelText("五笔第 2 候选键"), { target: { value: "'" } });
  expect((screen.getByRole("button", { name: "保存选词键" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect(onChange).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("五笔第 3 候选键"), { target: { value: ";" } });
  fireEvent.click(screen.getByRole("button", { name: "保存选词键" }));
  expect(onChange).toHaveBeenLastCalledWith({
    wubi_candidate_selection: { enabled: true, second_key: "'", third_key: ";" },
  });
});

test("五笔选词键拒绝空值、字母、数字和中文标点草稿", () => {
  const onChange = vi.fn();
  render(
    <WubiSection
      preferences={{ wubi_candidate_selection: { enabled: true, second_key: ";", third_key: "'" } }}
      onChange={onChange}
    />,
  );
  for (const value of ["", "a", "2", " ", "；", "'"]) {
    fireEvent.change(screen.getByLabelText("五笔第 2 候选键"), { target: { value } });
    expect((screen.getByRole("button", { name: "保存选词键" }) as HTMLButtonElement).disabled).toBe(
      true,
    );
  }
  expect(onChange).not.toHaveBeenCalled();
});
