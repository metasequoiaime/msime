/** Returns the platform-specific chord that toggles fullwidth character input. */
export function fullwidthShortcutChord(macos: boolean): string {
  return macos ? "Option+Shift+H" : "Alt+Shift+H";
}

/** Returns the platform-specific modifier prefix for maintenance shortcuts. */
export function maintenanceShortcutChord(macos: boolean): string {
  return macos ? "Ctrl+Shift+Option" : "Ctrl+Shift+Alt";
}
