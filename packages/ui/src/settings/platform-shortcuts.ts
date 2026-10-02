/** Returns the platform-specific chord that toggles fullwidth character input. */
export function fullwidthShortcutChord(macos: boolean): string {
  return macos ? "Option+Shift+H" : "Alt+Shift+H";
}
