// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type CardAuthorProps = {
  prefix?: string;
  author: string;
  owned: boolean;
  private?: boolean;
  removed?: boolean;
};

test("community card author shares prefix and ownership badges", () => {
  const Author = (ui as unknown as { CommunityCardAuthor: ComponentType<CardAuthorProps> })
    .CommunityCardAuthor;
  expect(Author).toBeDefined();

  render(<Author prefix="技术" author="合成用户" owned private removed />);

  expect(screen.getByText("技术 · 我的作品 · 私有 · 已下架")).toBeTruthy();
});

test("community card author shows another author's name", () => {
  const Author = (ui as unknown as { CommunityCardAuthor: ComponentType<CardAuthorProps> })
    .CommunityCardAuthor;
  render(<Author author="其他用户" owned={false} />);

  expect(screen.getByText("其他用户")).toBeTruthy();
});
