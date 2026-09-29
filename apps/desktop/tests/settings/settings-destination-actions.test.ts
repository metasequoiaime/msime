import { expect, test, vi } from "vitest";
import { useSettingsDestinationActions } from "@msime/ui";

test("creates shared about and community destinations", () => {
  const selectPage = vi.fn();
  const setShowTouchSkinEditor = vi.fn();
  const setCommunityDestination = vi.fn();
  const actions = useSettingsDestinationActions({
    selectPage,
    setShowTouchSkinEditor,
    setCommunityDestination,
  });

  actions.openAbout();
  actions.openCommunityAll();

  expect(selectPage).toHaveBeenNthCalledWith(1, "about");
  expect(selectPage).toHaveBeenNthCalledWith(2, "community");
  expect(setCommunityDestination).toHaveBeenCalledWith("all");
});

test("opens local designs and keeps the editor open", () => {
  const selectPage = vi.fn();
  const setShowTouchSkinEditor = vi.fn();
  const actions = useSettingsDestinationActions({
    selectPage,
    setShowTouchSkinEditor,
    setCommunityDestination: vi.fn(),
  });

  actions.openLocalDesigns();

  expect(selectPage).toHaveBeenCalledWith("skin");
  expect(setShowTouchSkinEditor).toHaveBeenCalledWith(true);
});
