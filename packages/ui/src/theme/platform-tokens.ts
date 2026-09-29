/**
 * Per-platform settings tokens: the design prototype's `tok(plat, dark)` as a typed table, for the eight settings looks the redesign draws (Windows, macOS, Linux, HarmonyOS 2-in-1, HarmonyOS phone, Android, iPhone, iPad), each in light and dark.
 *
 * The table is the source of truth. `styles.css` carries the same values as a `[data-platform]` layer above the upstream palette so they apply without a render, and `apps/desktop/tests/core/platform-tokens.test.ts` checks the two agree declaration for declaration. The upstream 64-variable palette underneath stays byte-equal to the source sheet; only the accent quartet and the new `--p-*` names are set here.
 *
 * Keyboard, candidate-window and device-frame tokens from the prototype are left out on purpose: those surfaces are drawn natively on every host, so nothing on the web side would read them.
 */

export type SettingsPlatform =
  | "win"
  | "mac"
  | "linux"
  | "hm2"
  | "harmony"
  | "android"
  | "ios"
  | "ipad";
export type SettingsAppearance = "light" | "dark";

export const settingsPlatforms: readonly SettingsPlatform[] = [
  "win",
  "mac",
  "linux",
  "hm2",
  "harmony",
  "android",
  "ios",
  "ipad",
];

/**
 * How a group of rows is drawn: `card` gives every row its own card with a gap between (Windows 11 settings), `inset` puts the rows in one rounded container with hairlines between them (macOS, GNOME, HarmonyOS, iOS), `separated` leaves the rows on the page with no container and no hairlines (Material 3).
 */
export type GroupListStyle = "card" | "inset" | "separated";

export interface PlatformTokens {
  font: string;
  /** The accent as a fill: switches, the selected check, the focus ring. */
  accent: string;
  /** The accent as text, which GNOME darkens in light mode for contrast. */
  accentText: string;
  /** A fill behind white text; in dark mode it stays the light-mode accent (see `accentPair`). */
  accentStrong: string;
  accentSoft: string;
  accentSoftBorder: string;
  /** The highlighted row in an open dropdown, the upstream `--dropdown-item-hover-bg`. */
  accentMenuHover: string;
  /** Foreground on an `accent` fill. */
  onAccent: string;
  text: string;
  sub: string;
  bg: string;
  chrome: string;
  hair: string;
  hover: string;
  controlRadius: string;
  switch: {
    width: string;
    height: string;
    knob: string;
    /** Material 3 shrinks the knob when off; every other platform keeps one size. */
    knobOffSize: string;
    pad: string;
    on: string;
    off: string;
    offRing: string;
    knobOn: string;
    knobOff: string;
    knobShadow: string;
    /** Windows prints 开/关 beside the switch. */
    label: boolean;
  };
  segmented: {
    bg: string;
    border: string;
    radius: string;
    itemRadius: string;
    on: string;
    onText: string;
    onShadow: string;
    /** Windows marks the chosen item with a short accent bar under it, 0 elsewhere. */
    indicatorWidth: string;
  };
  groupTitle: { size: string; weight: string; color: string; pad: string; gap: string };
  group: {
    style: GroupListStyle;
    bg: string;
    radius: string;
    shadow: string;
    rowGap: string;
    rowBg: string;
    /** The Windows card outline; `none` where rows share a container. */
    rowShadow: string;
    rowRadius: string;
    rowHeight: string;
    rowPad: string;
    rowSize: string;
    subSize: string;
    divider: boolean;
    rowIcon: boolean;
  };
  select: { bg: string; fg: string; border: string; pad: string; size: string; chevron: string };
  button: { bg: string; fg: string; border: string };
  pageTitle: { size: string; weight: string };
  groupsGap: string;
  slider: {
    /** Windows keeps the platform range control and only tints it. */
    native: boolean;
    trackHeight: string;
    track: string;
    knobWidth: string;
    knobHeight: string;
    knobRadius: string;
    knob: string;
    knobShadow: string;
  };
  nav: { height: string; radius: string; pad: string; selectedBg: string; pill: boolean };
  check: { size: string; radius: string };
}

const brand = { light: "#2C7A4B", dark: "#5FBF84" } as const;

function rgba(hex: string, alpha: number): string {
  const value = Number.parseInt(hex.slice(1, 7), 16);
  return `rgba(${(value >> 16) & 255}, ${(value >> 8) & 255}, ${value & 255}, ${alpha})`;
}

function tokensFor(platform: SettingsPlatform, dark: boolean): PlatformTokens {
  const S = <T>(light: T, darkValue: T): T => (dark ? darkValue : light);
  const appleFont =
    "-apple-system, BlinkMacSystemFont, 'PingFang SC', 'SF Pro Text', 'Noto Sans SC', sans-serif";
  const accentPair = (light: string, darkValue: string) => {
    const accent = S(light, darkValue);
    return {
      accent,
      accentText: accent,
      // `--accent-strong` is the fill under `text-white` across the shared page (primary buttons, the chosen chip, chat bubbles). The design pairs each accent with its own on-colour instead, and in dark mode that is black for Windows and deep green for Android, so a light dark-mode accent under the page's white text would be unreadable. The strong fill therefore stays the light-mode accent in both appearances, which carries white text on every platform.
      accentStrong: light,
      accentSoftBorder: rgba(accent, S(0.22, 0.25)),
      accentMenuHover: rgba(accent, S(0.12, 0.2)),
    };
  };
  const onAccent =
    platform === "win" && dark ? "#000000" : platform === "android" && dark ? "#003920" : "#FFFFFF";
  const nativeSlider = {
    native: false,
    trackHeight: "4px",
    track: S("rgba(120,120,128,.2)", "rgba(120,120,128,.36)"),
    knobWidth: "22px",
    knobHeight: "22px",
    knobRadius: "50%",
    knob: "#FFFFFF",
    knobShadow: "0 0 0 .5px rgba(0,0,0,.04), 0 3px 8px rgba(0,0,0,.15), 0 3px 1px rgba(0,0,0,.06)",
  };
  const check = { size: "18px", radius: "4px" };

  if (platform === "win") {
    const text = S("#1B1B1B", "#FFFFFF");
    const border = S("rgba(0,0,0,.08)", "rgba(255,255,255,.08)");
    const hover = S("rgba(0,0,0,.045)", "rgba(255,255,255,.07)");
    const accents = accentPair("#005FB8", "#60CDFF");
    return {
      font: "'Segoe UI Variable Text', 'Segoe UI', 'Microsoft YaHei UI', 'Noto Sans SC', sans-serif",
      ...accents,
      accentSoft: S("rgba(0,95,184,.1)", "rgba(96,205,255,.16)"),
      onAccent,
      text,
      sub: S("#5E5E5E", "rgba(255,255,255,.72)"),
      bg: S("#F3F3F3", "#202020"),
      chrome: S("#F3F3F3", "#202020"),
      hair: border,
      hover,
      controlRadius: "4px",
      switch: {
        width: "40px",
        height: "20px",
        knob: "12px",
        knobOffSize: "12px",
        pad: "4px",
        on: accents.accent,
        off: "transparent",
        offRing: `inset 0 0 0 1px ${S("rgba(0,0,0,.6)", "rgba(255,255,255,.6)")}`,
        knobOn: S("#FFFFFF", "#000000"),
        knobOff: S("rgba(0,0,0,.62)", "rgba(255,255,255,.8)"),
        knobShadow: "none",
        label: true,
      },
      segmented: {
        bg: S("rgba(0,0,0,.04)", "rgba(0,0,0,.12)"),
        border: `1px solid ${border}`,
        radius: "5px",
        itemRadius: "4px",
        on: S("rgba(0,0,0,.0578)", "rgba(255,255,255,.0837)"),
        onText: text,
        onShadow: "none",
        indicatorWidth: "16px",
      },
      groupTitle: { size: "14px", weight: "600", color: text, pad: "8px 2px 2px", gap: "4px" },
      group: {
        style: "card",
        bg: "transparent",
        radius: "0",
        shadow: "none",
        rowGap: "4px",
        rowBg: S("rgba(255,255,255,.7)", "rgba(255,255,255,.0512)"),
        rowShadow: S("0 0 0 1px rgba(0,0,0,.0578)", "0 0 0 1px rgba(0,0,0,.1)"),
        rowRadius: "4px",
        rowHeight: "62px",
        rowPad: "10px 20px",
        rowSize: "14px",
        subSize: "12px",
        divider: false,
        rowIcon: true,
      },
      select: {
        bg: S("#FFFFFF", "rgba(255,255,255,.06)"),
        fg: text,
        border: `1px solid ${border}`,
        pad: "5px 11px",
        size: "14px",
        chevron: "⌄",
      },
      button: {
        bg: S("#FFFFFF", "rgba(255,255,255,.06)"),
        fg: text,
        border: `1px solid ${border}`,
      },
      pageTitle: { size: "28px", weight: "600" },
      groupsGap: "8px",
      slider: { ...nativeSlider, native: true },
      nav: { height: "36px", radius: "4px", pad: "0 12px", selectedBg: hover, pill: true },
      check: { size: "20px", radius: "4px" },
    };
  }

  if (platform === "mac") {
    const text = S("#1D1D1F", "#F5F5F7");
    const hover = S("rgba(0,0,0,.06)", "rgba(255,255,255,.1)");
    const accents = accentPair(brand.light, brand.dark);
    return {
      font: appleFont,
      ...accents,
      accentSoft: S("rgba(44,122,75,.12)", "rgba(95,191,132,.22)"),
      onAccent,
      text,
      sub: S("#6E6E73", "#98989D"),
      bg: S("#FFFFFF", "#1E1E1E"),
      chrome: S("#F6F6F6", "#2A2A2A"),
      hair: S("rgba(0,0,0,.09)", "rgba(255,255,255,.1)"),
      hover,
      controlRadius: "6px",
      switch: {
        width: "38px",
        height: "22px",
        knob: "20px",
        knobOffSize: "20px",
        pad: "1px",
        on: accents.accent,
        off: S("rgba(0,0,0,.09)", "rgba(255,255,255,.16)"),
        offRing: S("inset 0 0 0 .5px rgba(0,0,0,.12)", "none"),
        knobOn: "#FFFFFF",
        knobOff: "#FFFFFF",
        knobShadow: "0 .5px 1.5px rgba(0,0,0,.35), 0 0 0 .5px rgba(0,0,0,.06)",
        label: false,
      },
      segmented: {
        bg: S("rgba(0,0,0,.055)", "rgba(255,255,255,.08)"),
        border: "none",
        radius: "6px",
        itemRadius: "5px",
        on: S("#FFFFFF", "#5A5A5E"),
        onText: text,
        onShadow: "0 0 0 .5px rgba(0,0,0,.1), 0 1px 1.5px rgba(0,0,0,.14)",
        indicatorWidth: "0",
      },
      groupTitle: { size: "14px", weight: "600", color: text, pad: "0 8px", gap: "8px" },
      group: {
        style: "inset",
        bg: S("#FFFFFF", "rgba(255,255,255,.045)"),
        radius: "10px",
        shadow: S("0 0 0 1px rgba(0,0,0,.06)", "0 0 0 1px rgba(255,255,255,.07)"),
        rowGap: "0",
        rowBg: "transparent",
        rowShadow: "none",
        rowRadius: "0",
        rowHeight: "56px",
        rowPad: "10px 16px",
        rowSize: "14px",
        subSize: "12px",
        divider: true,
        rowIcon: false,
      },
      select: {
        bg: "transparent",
        fg: text,
        border: "none",
        pad: "1px 2px",
        size: "14px",
        chevron: "⌃⌄",
      },
      button: {
        bg: S("#FFFFFF", "rgba(255,255,255,.14)"),
        fg: text,
        border: S(".5px solid rgba(0,0,0,.2)", ".5px solid rgba(255,255,255,.1)"),
      },
      pageTitle: { size: "26px", weight: "700" },
      groupsGap: "24px",
      slider: {
        ...nativeSlider,
        track: S("rgba(0,0,0,.1)", "rgba(255,255,255,.18)"),
        knobWidth: "20px",
        knobHeight: "20px",
        knobShadow: "0 0 0 .5px rgba(0,0,0,.2), 0 1px 2px rgba(0,0,0,.3)",
      },
      nav: { height: "36px", radius: "8px", pad: "0 10px", selectedBg: hover, pill: false },
      check,
    };
  }

  if (platform === "linux") {
    const text = S("rgba(0,0,0,.82)", "#FFFFFF");
    const hover = S("rgba(0,0,0,.06)", "rgba(255,255,255,.08)");
    const accents = accentPair("#3584E4", "#3584E4");
    return {
      font: "Cantarell, 'Noto Sans', 'Noto Sans SC', sans-serif",
      ...accents,
      accentText: S("#1C71D8", "#78AEED"),
      accentSoft: S("rgba(53,132,228,.14)", "rgba(120,174,237,.2)"),
      onAccent,
      text,
      sub: S("rgba(0,0,0,.55)", "rgba(255,255,255,.55)"),
      bg: S("#FAFAFA", "#242424"),
      chrome: S("#EBEBEB", "#303030"),
      hair: S("rgba(0,0,0,.1)", "rgba(0,0,0,.36)"),
      hover,
      controlRadius: "6px",
      switch: {
        width: "48px",
        height: "26px",
        knob: "20px",
        knobOffSize: "20px",
        pad: "3px",
        on: accents.accent,
        off: S("rgba(0,0,0,.15)", "rgba(255,255,255,.15)"),
        offRing: "none",
        knobOn: "#FFFFFF",
        knobOff: "#FFFFFF",
        knobShadow: "0 2px 4px rgba(0,0,0,.2)",
        label: false,
      },
      segmented: {
        bg: S("rgba(0,0,0,.08)", "rgba(255,255,255,.1)"),
        border: "none",
        radius: "8px",
        itemRadius: "6px",
        on: S("#FFFFFF", "rgba(255,255,255,.18)"),
        onText: text,
        onShadow: "0 1px 2px rgba(0,0,0,.15)",
        indicatorWidth: "0",
      },
      groupTitle: { size: "15px", weight: "700", color: text, pad: "0 2px", gap: "10px" },
      group: {
        style: "inset",
        bg: S("#FFFFFF", "rgba(255,255,255,.08)"),
        radius: "12px",
        shadow: `0 0 0 1px ${S("rgba(0,0,0,.08)", "rgba(0,0,0,.36)")}, 0 1px 3px rgba(0,0,0,.07)`,
        rowGap: "0",
        rowBg: "transparent",
        rowShadow: "none",
        rowRadius: "0",
        rowHeight: "54px",
        rowPad: "8px 14px",
        rowSize: "15px",
        subSize: "13px",
        divider: true,
        rowIcon: false,
      },
      select: {
        bg: S("rgba(0,0,0,.06)", "rgba(255,255,255,.1)"),
        fg: text,
        border: "none",
        pad: "6px 12px",
        size: "14px",
        chevron: "⌄",
      },
      button: { bg: S("rgba(0,0,0,.08)", "rgba(255,255,255,.1)"), fg: text, border: "none" },
      pageTitle: { size: "22px", weight: "800" },
      groupsGap: "24px",
      slider: {
        ...nativeSlider,
        trackHeight: "6px",
        track: S("rgba(0,0,0,.15)", "rgba(255,255,255,.15)"),
        knobWidth: "20px",
        knobHeight: "20px",
        knobShadow: "0 1px 3px rgba(0,0,0,.3)",
      },
      nav: { height: "40px", radius: "6px", pad: "0 12px", selectedBg: hover, pill: false },
      check: { size: "18px", radius: "6px" },
    };
  }

  if (platform === "ios" || platform === "ipad") {
    const text = S("#000000", "#FFFFFF");
    const hover = S("rgba(0,0,0,.05)", "rgba(255,255,255,.08)");
    const pad = platform === "ipad";
    const accents = accentPair(brand.light, brand.dark);
    return {
      font: appleFont,
      ...accents,
      accentSoft: S("rgba(44,122,75,.14)", "rgba(95,191,132,.26)"),
      onAccent,
      text,
      sub: S("#8A8A8E", "#8E8E93"),
      bg: S("#F2F2F7", "#000000"),
      chrome: S("#F2F2F7", "#000000"),
      hair: S("rgba(60,60,67,.22)", "rgba(84,84,88,.6)"),
      hover,
      controlRadius: "999px",
      switch: {
        width: "63px",
        height: "28px",
        knob: "24px",
        knobOffSize: "24px",
        pad: "2px",
        on: "#34C759",
        off: S("#E9E9EA", "#39393D"),
        offRing: "none",
        knobOn: "#FFFFFF",
        knobOff: "#FFFFFF",
        knobShadow: "0 2px 4px rgba(0,0,0,.2)",
        label: false,
      },
      segmented: {
        bg: S("rgba(118,118,128,.12)", "rgba(118,118,128,.24)"),
        border: "none",
        radius: "999px",
        itemRadius: "999px",
        on: S("#FFFFFF", "#636366"),
        onText: text,
        onShadow: "0 1px 3px rgba(0,0,0,.12)",
        indicatorWidth: "0",
      },
      groupTitle: {
        size: "13px",
        weight: "400",
        color: S("#6D6D72", "#8E8E93"),
        pad: "0 20px",
        gap: "7px",
      },
      group: {
        style: "inset",
        bg: S("#FFFFFF", "#1C1C1E"),
        radius: "26px",
        shadow: "none",
        rowGap: "0",
        rowBg: "transparent",
        rowShadow: "none",
        rowRadius: "0",
        rowHeight: "52px",
        rowPad: "8px 20px",
        rowSize: "17px",
        subSize: "13px",
        divider: true,
        rowIcon: false,
      },
      select: {
        bg: "transparent",
        fg: S("#8A8A8E", "#8E8E93"),
        border: "none",
        pad: "0",
        size: "17px",
        chevron: "›",
      },
      button: { bg: "transparent", fg: accents.accent, border: "none" },
      pageTitle: { size: pad ? "30px" : "34px", weight: "700" },
      groupsGap: "28px",
      slider: { ...nativeSlider, knobWidth: "28px", knobHeight: "28px" },
      nav: {
        height: pad ? "44px" : "52px",
        radius: "0",
        pad: pad ? "0 14px" : "0 20px",
        selectedBg: hover,
        pill: false,
      },
      check,
    };
  }

  if (platform === "android") {
    const text = S("#191C19", "#E1E3DE");
    const outline = S("#717970", "#8A9389");
    const accents = accentPair(brand.light, "#8FD5A6");
    const accentSoft = S("#CFE9D6", "#2A4F37");
    return {
      font: "Roboto, 'Noto Sans SC', 'Noto Sans', sans-serif",
      ...accents,
      accentSoft,
      onAccent,
      text,
      sub: S("#414941", "#C0C9BF"),
      bg: S("#F7FBF3", "#111411"),
      chrome: S("#F7FBF3", "#111411"),
      hair: S("#DDE5DB", "#2A2F2A"),
      hover: S("rgba(0,0,0,.05)", "rgba(255,255,255,.06)"),
      controlRadius: "20px",
      switch: {
        width: "52px",
        height: "32px",
        knob: "24px",
        knobOffSize: "16px",
        pad: "4px",
        on: accents.accent,
        off: S("#DFE4DB", "#323532"),
        offRing: `inset 0 0 0 2px ${outline}`,
        knobOn: S("#FFFFFF", "#003920"),
        knobOff: outline,
        knobShadow: "none",
        label: false,
      },
      segmented: {
        bg: "transparent",
        border: `1px solid ${outline}`,
        radius: "999px",
        itemRadius: "999px",
        on: accentSoft,
        onText: text,
        onShadow: "none",
        indicatorWidth: "0",
      },
      groupTitle: { size: "14px", weight: "500", color: accents.accent, pad: "0 24px", gap: "2px" },
      group: {
        style: "separated",
        bg: "transparent",
        radius: "0",
        shadow: "none",
        rowGap: "0",
        rowBg: "transparent",
        rowShadow: "none",
        rowRadius: "0",
        rowHeight: "64px",
        rowPad: "8px 24px",
        rowSize: "16px",
        subSize: "14px",
        divider: false,
        rowIcon: false,
      },
      select: {
        bg: "transparent",
        fg: S("#414941", "#C0C9BF"),
        border: "none",
        pad: "0",
        size: "14px",
        chevron: "▾",
      },
      button: { bg: accentSoft, fg: text, border: "none" },
      pageTitle: { size: "28px", weight: "400" },
      groupsGap: "20px",
      slider: {
        native: false,
        trackHeight: "6px",
        track: accentSoft,
        knobWidth: "4px",
        knobHeight: "28px",
        knobRadius: "2px",
        knob: accents.accent,
        knobShadow: "none",
      },
      nav: { height: "56px", radius: "28px", pad: "0 24px", selectedBg: accentSoft, pill: false },
      check: { size: "18px", radius: "2px" },
    };
  }

  // HarmonyOS: the phone look is the base, and the 2-in-1 look overrides it the way the prototype's `tok('hm2')` spreads `tok('harmony')`.
  const text = S("#182431", "#E5E5E5");
  const hover = S("rgba(0,0,0,.05)", "rgba(255,255,255,.08)");
  const accents = accentPair(brand.light, brand.dark);
  const sub = S("rgba(24,36,49,.6)", "rgba(255,255,255,.6)");
  const harmony: PlatformTokens = {
    font: "'HarmonyOS Sans SC', 'HarmonyOS Sans', 'Noto Sans SC', sans-serif",
    ...accents,
    accentSoft: S("rgba(44,122,75,.12)", "rgba(95,191,132,.26)"),
    onAccent,
    text,
    sub,
    bg: S("#F1F3F5", "#000000"),
    chrome: S("#F1F3F5", "#000000"),
    hair: S("rgba(0,0,0,.06)", "rgba(255,255,255,.1)"),
    hover,
    controlRadius: "20px",
    switch: {
      width: "36px",
      height: "20px",
      knob: "16px",
      knobOffSize: "16px",
      pad: "2px",
      on: accents.accent,
      off: S("rgba(0,0,0,.1)", "rgba(255,255,255,.2)"),
      offRing: "none",
      knobOn: "#FFFFFF",
      knobOff: "#FFFFFF",
      knobShadow: "0 1px 2px rgba(0,0,0,.2)",
      label: false,
    },
    segmented: {
      bg: S("rgba(0,0,0,.05)", "rgba(255,255,255,.1)"),
      border: "none",
      radius: "999px",
      itemRadius: "999px",
      on: S("#FFFFFF", "#3A3A3A"),
      onText: text,
      onShadow: "0 1px 3px rgba(0,0,0,.12)",
      indicatorWidth: "0",
    },
    groupTitle: { size: "14px", weight: "500", color: sub, pad: "0 16px", gap: "8px" },
    group: {
      style: "inset",
      bg: S("#FFFFFF", "#1F1F1F"),
      radius: "20px",
      shadow: "none",
      rowGap: "0",
      rowBg: "transparent",
      rowShadow: "none",
      rowRadius: "0",
      rowHeight: "56px",
      rowPad: "8px 16px",
      rowSize: "16px",
      subSize: "14px",
      divider: true,
      rowIcon: false,
    },
    select: { bg: "transparent", fg: sub, border: "none", pad: "0", size: "14px", chevron: "›" },
    button: {
      bg: S("rgba(0,0,0,.05)", "rgba(255,255,255,.1)"),
      fg: accents.accent,
      border: "none",
    },
    pageTitle: { size: "30px", weight: "700" },
    groupsGap: "20px",
    slider: nativeSlider,
    nav: { height: "52px", radius: "0", pad: "0 14px", selectedBg: hover, pill: false },
    check,
  };
  if (platform === "harmony") return harmony;
  return {
    ...harmony,
    bg: S("#FFFFFF", "#121212"),
    chrome: S("#F1F3F5", "#1A1A1A"),
    group: {
      ...harmony.group,
      rowHeight: "52px",
      rowPad: "8px 16px",
      rowSize: "15px",
      subSize: "12px",
    },
    groupTitle: { ...harmony.groupTitle, pad: "0 12px" },
    select: {
      bg: S("rgba(0,0,0,.05)", "rgba(255,255,255,.1)"),
      fg: text,
      border: "none",
      pad: "6px 12px",
      size: "14px",
      chevron: "⌄",
    },
    button: {
      bg: S("rgba(0,0,0,.05)", "rgba(255,255,255,.1)"),
      fg: S(accents.accent, "#8FB6FF"),
      border: "none",
    },
    pageTitle: { size: "22px", weight: "700" },
    nav: { height: "40px", radius: "12px", pad: "0 10px", selectedBg: hover, pill: false },
  };
}

export const platformTokens: Readonly<
  Record<SettingsPlatform, Readonly<Record<SettingsAppearance, PlatformTokens>>>
> = Object.fromEntries(
  settingsPlatforms.map((platform) => [
    platform,
    { light: tokensFor(platform, false), dark: tokensFor(platform, true) },
  ]),
) as Record<SettingsPlatform, Record<SettingsAppearance, PlatformTokens>>;

/**
 * The custom properties one platform and appearance set on the settings root: the upstream accent variables (so every existing `accent` utility and the dropdown highlight follow the platform) plus the `--p-*` names the platform primitives in `core/platform-controls.tsx` read. The structural flags are emitted as the CSS value they switch (a `display`, an `appearance`, a width), so the primitives change shape per platform from the stylesheet alone and never need to be told which platform they are on.
 */
export function platformCssVariables(tokens: PlatformTokens): Record<string, string> {
  const {
    switch: sw,
    segmented: seg,
    groupTitle: title,
    group,
    select,
    button,
    slider,
    nav,
    check,
  } = tokens;
  return {
    "--accent-color": tokens.accent,
    "--accent-strong": tokens.accentStrong,
    "--accent-soft": tokens.accentSoft,
    "--accent-soft-border": tokens.accentSoftBorder,
    "--dropdown-item-hover-bg": tokens.accentMenuHover,
    "--p-font": tokens.font,
    "--p-accent-text": tokens.accentText,
    "--p-on-accent": tokens.onAccent,
    "--p-text": tokens.text,
    "--p-sub": tokens.sub,
    "--p-bg": tokens.bg,
    "--p-chrome": tokens.chrome,
    "--p-hair": tokens.hair,
    "--p-hover": tokens.hover,
    "--p-r-ctl": tokens.controlRadius,
    "--p-sw-w": sw.width,
    "--p-sw-h": sw.height,
    "--p-sw-knob": sw.knob,
    "--p-sw-knob-off-size": sw.knobOffSize,
    "--p-sw-pad": sw.pad,
    "--p-sw-on": sw.on,
    "--p-sw-off": sw.off,
    "--p-sw-off-ring": sw.offRing,
    "--p-sw-knob-on": sw.knobOn,
    "--p-sw-knob-off": sw.knobOff,
    "--p-sw-knob-shadow": sw.knobShadow,
    "--p-sw-label": sw.label ? "inline" : "none",
    "--p-seg-bg": seg.bg,
    "--p-seg-border": seg.border,
    "--p-seg-r": seg.radius,
    "--p-seg-item-r": seg.itemRadius,
    "--p-seg-on": seg.on,
    "--p-seg-on-fg": seg.onText,
    "--p-seg-on-shadow": seg.onShadow,
    "--p-seg-indicator": seg.indicatorWidth,
    "--p-g-title-fs": title.size,
    "--p-g-title-w": title.weight,
    "--p-g-title-fg": title.color,
    "--p-g-title-pad": title.pad,
    "--p-g-title-gap": title.gap,
    "--p-group-bg": group.bg,
    "--p-group-r": group.radius,
    "--p-group-shadow": group.shadow,
    "--p-row-gap": group.rowGap,
    "--p-row-bg": group.rowBg,
    "--p-row-shadow": group.rowShadow,
    "--p-row-r": group.rowRadius,
    "--p-row-h": group.rowHeight,
    "--p-row-pad": group.rowPad,
    "--p-row-fs": group.rowSize,
    "--p-sub-fs": group.subSize,
    "--p-row-divider": group.divider ? tokens.hair : "transparent",
    "--p-row-icon": group.rowIcon ? "flex" : "none",
    "--p-sel-bg": select.bg,
    "--p-sel-fg": select.fg,
    "--p-sel-border": select.border,
    "--p-sel-pad": select.pad,
    "--p-sel-fs": select.size,
    "--p-btn-bg": button.bg,
    "--p-btn-fg": button.fg,
    "--p-btn-border": button.border,
    "--p-title-fs": tokens.pageTitle.size,
    "--p-title-w": tokens.pageTitle.weight,
    "--p-groups-gap": tokens.groupsGap,
    "--p-slider-appearance": slider.native ? "auto" : "none",
    "--p-slider-track-h": slider.trackHeight,
    "--p-slider-track": slider.track,
    "--p-slider-knob-w": slider.knobWidth,
    "--p-slider-knob-h": slider.knobHeight,
    "--p-slider-knob-r": slider.knobRadius,
    "--p-slider-knob": slider.knob,
    "--p-slider-knob-shadow": slider.knobShadow,
    "--p-nav-h": nav.height,
    "--p-nav-r": nav.radius,
    "--p-nav-pad": nav.pad,
    "--p-nav-selected-bg": nav.selectedBg,
    "--p-nav-pill": nav.pill ? "3px" : "0",
    "--p-check-size": check.size,
    "--p-check-r": check.radius,
  };
}

/**
 * Which settings look a host gets. Harmony's 2-in-1 reports `mobile_settings: false` and gets the desktop-style `hm2`; an iPhone and an iPad are told apart by viewport width, because the host contract carries no idiom yet, at the same 601px the stylesheet's phone breakpoint uses.
 *
 * A page with no host at all (the browser fixtures, or a host that predates the capability contract) is the Windows reference the upstream palette came from, unless the user agent says Linux.
 */
export function settingsPlatformOf(
  host: { platform: string; mobile_settings?: boolean } | undefined,
  options: { wide: boolean; linuxUserAgent: boolean },
): SettingsPlatform {
  switch (host?.platform) {
    case "windows":
      return "win";
    case "macos":
      return "mac";
    case "linux":
      return "linux";
    case "android":
      return "android";
    case "ios":
      return options.wide ? "ipad" : "ios";
    case "harmony":
      return host.mobile_settings === false ? "hm2" : "harmony";
    default:
      return options.linuxUserAgent ? "linux" : "win";
  }
}
