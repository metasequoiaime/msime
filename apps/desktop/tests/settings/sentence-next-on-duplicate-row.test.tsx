// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SentenceAssociationSection } from "../../../../packages/ui/src/settings/sentence-association-section";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("桌面整句联想组提供「去重后补充次选」开关", () => {
  const onChange = vi.fn();
  render(<SentenceAssociationSection value={undefined} mobile={false} onChange={onChange} />);

  // 行标题与 Windows 侧的设置名一致；开关按行标题命名。
  fireEvent.click(screen.getByRole("switch", { name: "去重后补充次选" }));

  // 宿主只收到改变的那个字段：缺键文档打开后写进 `sentence_association.show_next_on_duplicate`。
  expect(onChange).toHaveBeenCalledTimes(1);
  expect(onChange).toHaveBeenCalledWith({ show_next_on_duplicate: true });
});

test("切换「去重后补充次选」保留整句联想里的其他字段", () => {
  const onChange = vi.fn();
  render(
    <SentenceAssociationSection
      value={{ word_lattice: true, neural_desktop: true, show_next_on_duplicate: false }}
      mobile={false}
      onChange={onChange}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "去重后补充次选" }));

  expect(onChange).toHaveBeenCalledWith({
    word_lattice: true,
    neural_desktop: true,
    show_next_on_duplicate: true,
  });
});
