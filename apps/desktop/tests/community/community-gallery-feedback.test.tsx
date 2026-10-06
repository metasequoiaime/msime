// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ComponentType, ReactNode } from "react";
import * as ui from "@msime/ui";

afterEach(cleanup);

test("shared gallery feedback hides empty content when the gallery failed", () => {
  const Feedback = (
    ui as unknown as {
      CommunityGalleryFeedback?: ComponentType<{
        error?: string;
        signInRequired?: boolean;
        onLogin?: () => void;
        notice?: ReactNode;
        empty?: ReactNode;
      }>;
    }
  ).CommunityGalleryFeedback;
  expect(Feedback).toBeDefined();
  if (!Feedback) return;

  const onLogin = vi.fn();
  render(
    <Feedback
      error="需要登录"
      signInRequired
      onLogin={onLogin}
      notice={<p>刚刚完成</p>}
      empty={<p>这里是空的</p>}
    />,
  );

  expect(screen.getByRole("alert").textContent).toContain("需要登录");
  expect(screen.getByText("刚刚完成")).toBeTruthy();
  expect(screen.queryByText("这里是空的")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "去登录" }));
  expect(onLogin).toHaveBeenCalledOnce();
});

test("shared gallery feedback renders empty content without an error", () => {
  const Feedback = (
    ui as unknown as {
      CommunityGalleryFeedback?: ComponentType<{ empty?: ReactNode }>;
    }
  ).CommunityGalleryFeedback;
  expect(Feedback).toBeDefined();
  if (!Feedback) return;

  render(<Feedback empty={<p>这里是空的</p>} />);
  expect(screen.getByText("这里是空的")).toBeTruthy();
});

test("community galleries reuse the shared feedback component", () => {
  const sources = [
    Object.values(
      import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
        eager: true,
        query: "?raw",
        import: "default",
      }),
    )[0],
    Object.values(
      import.meta.glob<string>("../../../../packages/ui/src/community/community-skins.tsx", {
        eager: true,
        query: "?raw",
        import: "default",
      }),
    )[0],
    Object.values(
      import.meta.glob<string>(
        "../../../../packages/ui/src/community/community-candidate-skins.tsx",
        { eager: true, query: "?raw", import: "default" },
      ),
    )[0],
    Object.values(
      import.meta.glob<string>("../../../../packages/ui/src/community/community-plugins.tsx", {
        eager: true,
        query: "?raw",
        import: "default",
      }),
    )[0],
  ];

  for (const source of sources) {
    expect(source).toContain(
      'import { CommunityGalleryFeedback } from "./community-gallery-feedback";',
    );
    expect(source).toContain("<CommunityGalleryFeedback");
    expect(source).not.toContain("CommunityErrorAlert");
  }
});
