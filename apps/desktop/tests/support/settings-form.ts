import { fireEvent, screen } from "@testing-library/react";

/** Resolves once the loaded settings form is on screen, the point from which sidebar clicks are no longer overtaken by the initial page selection. */
export async function settingsFormReady(): Promise<HTMLElement> {
  return screen.findByRole("form", { name: "设置" });
}

/** Saves the pending edits now instead of waiting out the autosave delay, the way leaving the window does. */
export function saveSettingsNow(): void {
  fireEvent.blur(window);
}
