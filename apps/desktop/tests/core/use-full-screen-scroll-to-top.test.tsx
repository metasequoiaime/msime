// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useFullScreenScrollToTop } from "../../../../packages/ui/src/core/use-full-screen-scroll-to-top";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  delete (HTMLElement.prototype as { scrollIntoView?: unknown }).scrollIntoView;
});

function mockScrollIntoView() {
  const scrollIntoView = vi.fn();
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: scrollIntoView,
  });
  return scrollIntoView;
}

function Surface({ enabled = true }: { enabled?: boolean }) {
  const surface = useFullScreenScrollToTop<HTMLDivElement>(enabled);
  return <div ref={surface} />;
}

test("scrolls a full-screen surface to the top when it becomes active", () => {
  const scrollIntoView = mockScrollIntoView();

  render(<Surface />);

  expect(scrollIntoView).toHaveBeenCalledOnce();
  expect(scrollIntoView).toHaveBeenCalledWith({ block: "start" });
});

test("does not scroll a full-screen surface while inactive", () => {
  const scrollIntoView = mockScrollIntoView();

  render(<Surface enabled={false} />);

  expect(scrollIntoView).not.toHaveBeenCalled();
});
