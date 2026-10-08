// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { useCommunityDetailHistory } from "@msime/ui";

function Probe({
  mobile,
  kind,
  selectedId,
  onClose,
}: {
  mobile: boolean;
  kind: string;
  selectedId: string | null;
  onClose: () => void;
}) {
  useCommunityDetailHistory({ mobile, kind, selectedId, onClose });
  return null;
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("closes a mobile community detail when history leaves its route", () => {
  const onClose = vi.fn();
  render(<Probe mobile kind="skin" selectedId="skin-1" onClose={onClose} />);

  window.dispatchEvent(
    new PopStateEvent("popstate", {
      state: { communityDetail: { kind: "resource", id: "resource-1" } },
    }),
  );

  expect(onClose).toHaveBeenCalledOnce();
});

test("ignores matching routes and desktop history", () => {
  const onClose = vi.fn();
  const { rerender } = render(<Probe mobile kind="skin" selectedId="skin-1" onClose={onClose} />);

  window.dispatchEvent(
    new PopStateEvent("popstate", {
      state: { communityDetail: { kind: "skin", id: "skin-1" } },
    }),
  );
  expect(onClose).not.toHaveBeenCalled();

  rerender(<Probe mobile={false} kind="skin" selectedId="skin-1" onClose={onClose} />);
  window.dispatchEvent(new PopStateEvent("popstate", { state: {} }));
  expect(onClose).not.toHaveBeenCalled();
});

test("uses the current detail and close action after rerender", () => {
  const oldClose = vi.fn();
  const currentClose = vi.fn();
  const { rerender } = render(<Probe mobile kind="skin" selectedId="skin-1" onClose={oldClose} />);
  rerender(<Probe mobile kind="reply" selectedId="reply-2" onClose={currentClose} />);

  window.dispatchEvent(
    new PopStateEvent("popstate", {
      state: { communityDetail: { kind: "reply", id: "reply-2" } },
    }),
  );
  expect(currentClose).not.toHaveBeenCalled();

  window.dispatchEvent(new PopStateEvent("popstate", { state: {} }));
  expect(currentClose).toHaveBeenCalledOnce();
  expect(oldClose).not.toHaveBeenCalled();
});

test("ignores history when no detail is selected and after unmount", () => {
  const onClose = vi.fn();
  const { rerender, unmount } = render(
    <Probe mobile kind="skin" selectedId={null} onClose={onClose} />,
  );
  window.dispatchEvent(new PopStateEvent("popstate", { state: {} }));
  expect(onClose).not.toHaveBeenCalled();

  rerender(<Probe mobile kind="skin" selectedId="skin-1" onClose={onClose} />);
  unmount();
  window.dispatchEvent(new PopStateEvent("popstate", { state: {} }));
  expect(onClose).not.toHaveBeenCalled();
});
