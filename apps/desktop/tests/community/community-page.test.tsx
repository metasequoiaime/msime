// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

afterEach(cleanup);

vi.mock("../../../../packages/ui/src/community/community-skins", () => ({
  CommunitySkinsPage: (props: Record<string, unknown>) => (
    <div data-testid="skins-page">{JSON.stringify(props)}</div>
  ),
}));

vi.mock("../../../../packages/ui/src/community/community-resources", () => ({
  CommunityHomePage: (props: Record<string, unknown>) => (
    <div data-testid="home-page">{JSON.stringify(props)}</div>
  ),
  CommunityResourcesPage: (props: Record<string, unknown>) => (
    <div data-testid="resources-page">{JSON.stringify(props)}</div>
  ),
}));

import { CommunityPage } from "../../../../packages/ui/src/community/community-page";

const common = {
  theme: "dark" as const,
  initialMine: true,
  initialCategory: "reply" as const,
  initialScope: "saved" as const,
  localDictionary: { import: vi.fn() } as never,
  localSkinLibrary: { load: vi.fn(), mutate: vi.fn() } as never,
  mobile: true,
  onLogin: vi.fn(),
  destinationKey: "saved-reply",
};

test("routes both community clients through the combined page", () => {
  render(
    <CommunityPage
      {...common}
      skins={{} as never}
      resources={{} as never}
    />,
  );

  expect(screen.getByTestId("home-page")).toBeTruthy();
  expect(screen.queryByTestId("skins-page")).toBeNull();
  expect(screen.queryByTestId("resources-page")).toBeNull();
});

test("routes a skins-only client to the skin page", () => {
  render(<CommunityPage {...common} skins={{} as never} />);

  expect(screen.getByTestId("skins-page")).toBeTruthy();
  expect(screen.queryByTestId("home-page")).toBeNull();
  expect(screen.queryByTestId("resources-page")).toBeNull();
});

test("routes a resources-only client to the requested resource kind", () => {
  render(<CommunityPage {...common} resources={{} as never} />);

  expect(screen.getByTestId("resources-page")).toBeTruthy();
  expect(screen.queryByTestId("home-page")).toBeNull();
  expect(screen.queryByTestId("skins-page")).toBeNull();
  expect(screen.getByTestId("resources-page").textContent).toContain('"kind":"reply"');
});
