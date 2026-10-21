import type { FloatingToolbarPreferences } from "../settings/floating-toolbar-preferences";

export const toolbarImageKeys = [
  "status",
  "fullwidth",
  "punctuation",
  "character_set",
  "emoji",
  "screen_keyboard",
  "settings",
] as const;
export type ToolbarImageKey = (typeof toolbarImageKeys)[number];
export type ToolbarButtonLook = {
  widthDip?: number | null;
  heightDip?: number | null;
  iconSizeDip?: number | null;
  background?: string | null;
};
export type ToolbarImageButton = ToolbarButtonLook & {
  visible?: boolean | null;
  xDip?: number | null;
  yDip?: number | null;
  image?: string | null;
  images?: Record<string, string>;
};
export type SkinToolbarImage = {
  image: string;
  imageWidthDip: number;
  imageHeightDip: number;
  buttonDefaults: ToolbarButtonLook;
  buttonFlow: {
    xDip: number;
    yDip: number;
    direction: "horizontal" | "vertical";
    gapDip: number;
  } | null;
  buttons: Partial<Record<ToolbarImageKey, ToolbarImageButton>>;
};
export type ToolbarImageSlot = {
  key: ToolbarImageKey;
  xDip: number;
  yDip: number;
  widthDip: number;
  heightDip: number;
  iconSizeDip: number;
  background: string | null;
  image: string | null;
};

/** 共享扫描结果的浏览器布局，与 catalog::SkinToolbarImage::layout 保持同一规则。 */
export function toolbarImageLayout(
  skin: SkinToolbarImage,
  shown: ReadonlyArray<readonly [ToolbarImageKey, string]>,
): { widthDip: number; heightDip: number; buttons: ToolbarImageSlot[] } {
  const result = {
    widthDip: skin.imageWidthDip,
    heightDip: skin.imageHeightDip,
    buttons: [] as ToolbarImageSlot[],
  };
  const defaults = skin.buttonDefaults;
  const flow = skin.buttonFlow;
  let x = flow?.xDip ?? 0;
  let y = flow?.yDip ?? 0;
  for (const [key, state] of shown) {
    if (result.buttons.some((slot) => slot.key === key)) continue;
    const button = skin.buttons[key] ?? {};
    if (button.visible === false) continue;
    const widthDip = button.widthDip ?? defaults.widthDip ?? 26;
    const heightDip = button.heightDip ?? defaults.heightDip ?? 26;
    let xDip: number;
    let yDip: number;
    if (button.xDip != null && button.yDip != null) {
      xDip = button.xDip;
      yDip = button.yDip;
    } else if (flow) {
      xDip = x;
      yDip = y;
      if (flow.direction === "vertical") y += heightDip + flow.gapDip;
      else x += widthDip + flow.gapDip;
    } else continue;
    result.widthDip = Math.max(result.widthDip, xDip + widthDip);
    result.heightDip = Math.max(result.heightDip, yDip + heightDip);
    result.buttons.push({
      key,
      xDip,
      yDip,
      widthDip,
      heightDip,
      iconSizeDip:
        button.iconSizeDip ?? defaults.iconSizeDip ?? Math.min(widthDip, heightDip) * 0.82,
      background: button.background ?? defaults.background ?? null,
      image: button.images?.[state] ?? button.image ?? null,
    });
  }
  return result;
}

/** 预览展示中文、半角、中文标点与简体；按钮仍遵循用户的显示开关。 */
export function toolbarPreviewRequests(
  preferences?: FloatingToolbarPreferences,
): Array<[ToolbarImageKey, string]> {
  const states = {
    status: "cn",
    fullwidth: "half",
    punctuation: "cn",
    character_set: "simplified",
    emoji: "",
    screen_keyboard: "",
    settings: "",
  };
  return toolbarImageKeys
    .filter((key) => !preferences || preferences[key === "status" ? "english_mode" : key])
    .map((key): [ToolbarImageKey, string] => [key, states[key]]);
}
