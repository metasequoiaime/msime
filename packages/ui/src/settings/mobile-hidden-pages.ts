export type MobileHiddenPageId = "shortcuts" | "floating-toolbar" | "plugins";

export function mobileHiddenPageIds(options: {
  modeSwitchShortcuts: boolean;
  panelShortcuts: boolean;
  desktopMaintenanceShortcuts: boolean;
}): readonly MobileHiddenPageId[] {
  return [
    ...(options.modeSwitchShortcuts || options.panelShortcuts || options.desktopMaintenanceShortcuts
      ? []
      : (["shortcuts"] as const)),
    "floating-toolbar",
    // Sound packs, music and the / and @ modes belong to a physical keyboard; a phone keeps its own keyboard feedback settings.
    "plugins",
  ];
}
