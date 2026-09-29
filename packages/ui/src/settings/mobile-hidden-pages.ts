export type MobileHiddenPageId = "shortcuts" | "floating-toolbar" | "helpcode";

export function mobileHiddenPageIds(options: {
  modeSwitchShortcuts: boolean;
  panelShortcuts: boolean;
  desktopMaintenanceShortcuts: boolean;
  helpcodeShiftEntry: boolean;
  android: boolean;
  harmony: boolean;
}): readonly MobileHiddenPageId[] {
  return [
    ...(options.modeSwitchShortcuts || options.panelShortcuts || options.desktopMaintenanceShortcuts
      ? []
      : (["shortcuts"] as const)),
    "floating-toolbar",
    ...(options.helpcodeShiftEntry || options.android || options.harmony
      ? []
      : (["helpcode"] as const)),
  ];
}
