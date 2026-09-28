import { useId } from "react";
import {
  readableSkinText,
  skinColor,
  type TouchKeyboardSkinDesign,
} from "./touch-keyboard-skin-design";
import { actionKeyboardLabels, desktopKeyboardRows, touchKeyboardRows } from "./keyboard-layouts";
import { keyboardKeyPath } from "./keyboard-shape";

// Built-in visual source: MSIME-Apple@11c950a63ec57656cd78b3f75aa621c293bfe453,
// platforms/ios/SharedUI/KeyboardSkinPreference.swift and KeyboardSkinBackgroundView.swift.
export type TouchKeyboardSkin =
  | "forest"
  | "ocean"
  | "rose"
  | "porcelain"
  | "typewriter"
  | "candy"
  | "midnight"
  | "blueprint"
  | "custom";

export type TouchKeyboardSkinPalette = {
  background: string;
  key: string;
  foreground: string;
  accent: string;
  action: string;
};
export type TouchKeyboardSkinOption = {
  id: TouchKeyboardSkin;
  title: string;
  description: string;
  light: TouchKeyboardSkinPalette;
  dark: TouchKeyboardSkinPalette;
  cornerRadius: number;
  borderWidth: number;
  shadowOpacity: number;
  shadowRadius: number;
  shadowOffset: number;
  monospaced: boolean;
  pattern: 0 | 1 | 2 | 3;
};

export const touchKeyboardSkinOptions: TouchKeyboardSkinOption[] = [
  {
    id: "forest",
    title: "水杉绿",
    description: "清新留白 · 经典圆角",
    light: {
      background: "#e8f0eb",
      key: "#fff",
      foreground: "#000",
      accent: "#185c47",
      action: "#185c47",
    },
    dark: {
      background: "#17211c",
      key: "#303d36",
      foreground: "#fff",
      accent: "#73cca6",
      action: "#1f614a",
    },
    cornerRadius: 8,
    borderWidth: 0,
    shadowOpacity: 0,
    shadowRadius: 3,
    shadowOffset: 2,
    monospaced: false,
    pattern: 0,
  },
  {
    id: "ocean",
    title: "海盐蓝",
    description: "海盐浅蓝 · 轻盈平面",
    light: {
      background: "#e6f0fa",
      key: "#fff",
      foreground: "#000",
      accent: "#1f5ca3",
      action: "#1f5ca3",
    },
    dark: {
      background: "#171f2b",
      key: "#2e384a",
      foreground: "#fff",
      accent: "#80bdfa",
      action: "#295c9e",
    },
    cornerRadius: 8,
    borderWidth: 0,
    shadowOpacity: 0,
    shadowRadius: 3,
    shadowOffset: 2,
    monospaced: false,
    pattern: 0,
  },
  {
    id: "rose",
    title: "浅蔷薇",
    description: "柔和蔷薇 · 简洁圆角",
    light: {
      background: "#fae8f0",
      key: "#fff",
      foreground: "#000",
      accent: "#a14063",
      action: "#a14063",
    },
    dark: {
      background: "#291a21",
      key: "#45303b",
      foreground: "#fff",
      accent: "#f59ebd",
      action: "#8f3b5c",
    },
    cornerRadius: 8,
    borderWidth: 0,
    shadowOpacity: 0,
    shadowRadius: 3,
    shadowOffset: 2,
    monospaced: false,
    pattern: 0,
  },
  {
    id: "porcelain",
    title: "素白瓷",
    description: "细线边框 · 克制直角",
    light: {
      background: "#ebedf0",
      key: "#fcfcfc",
      foreground: "#000",
      accent: "#333d47",
      action: "#333d47",
    },
    dark: {
      background: "#1a1c21",
      key: "#33363b",
      foreground: "#fff",
      accent: "#ccd6e3",
      action: "#454f5c",
    },
    cornerRadius: 3,
    borderWidth: 0.5,
    shadowOpacity: 0,
    shadowRadius: 3,
    shadowOffset: 2,
    monospaced: false,
    pattern: 0,
  },
  {
    id: "typewriter",
    title: "纸上时光",
    description: "暖纸网点 · 复古键帽",
    light: {
      background: "#e3d6bd",
      key: "#fcf5e0",
      foreground: "#000",
      accent: "#5e4026",
      action: "#5e4026",
    },
    dark: {
      background: "#26211a",
      key: "#40382b",
      foreground: "#fff",
      accent: "#deb882",
      action: "#66472e",
    },
    cornerRadius: 5,
    borderWidth: 1,
    shadowOpacity: 0.3,
    shadowRadius: 0,
    shadowOffset: 3,
    monospaced: true,
    pattern: 1,
  },
  {
    id: "candy",
    title: "奶油桃桃",
    description: "奶油波纹 · 饱满圆角",
    light: {
      background: "#fce0d1",
      key: "#fff7ed",
      foreground: "#000",
      accent: "#943852",
      action: "#943852",
    },
    dark: {
      background: "#301f26",
      key: "#4d333d",
      foreground: "#fff",
      accent: "#ffa8ba",
      action: "#943852",
    },
    cornerRadius: 18,
    borderWidth: 0,
    shadowOpacity: 0.16,
    shadowRadius: 3,
    shadowOffset: 2,
    monospaced: false,
    pattern: 3,
  },
  {
    id: "midnight",
    title: "霓虹夜航",
    description: "紫色星点 · 霓虹描边",
    light: {
      background: "#130f24",
      key: "#291f40",
      foreground: "#fff",
      accent: "#c7b0ff",
      action: "#663bb3",
    },
    dark: {
      background: "#130f24",
      key: "#291f40",
      foreground: "#fff",
      accent: "#c7b0ff",
      action: "#663bb3",
    },
    cornerRadius: 10,
    borderWidth: 1,
    shadowOpacity: 0,
    shadowRadius: 3,
    shadowOffset: 2,
    monospaced: false,
    pattern: 1,
  },
  {
    id: "blueprint",
    title: "工程蓝图",
    description: "蓝图网格 · 等宽字形",
    light: {
      background: "#0e2138",
      key: "#173352",
      foreground: "#fff",
      accent: "#8ad6ff",
      action: "#1f578a",
    },
    dark: {
      background: "#0e2138",
      key: "#173352",
      foreground: "#fff",
      accent: "#8ad6ff",
      action: "#1f578a",
    },
    cornerRadius: 3,
    borderWidth: 1,
    shadowOpacity: 0,
    shadowRadius: 3,
    shadowOffset: 2,
    monospaced: true,
    pattern: 2,
  },
];

// Layout source: MSIME-Windows@04a8df56f86312474a069f4335a1b58da7afaa9e,
// server/src/keyboard-panel/KeyboardPanel.cpp (GPL-3.0). This preview has no input actions.
// The touch hosts put three letter rows above a control strip, so the desktop artwork above is the
// wrong picture of them: it promises a number row, Tab, Caps Lock and Win keys that a phone keyboard
// simply does not have. Mirrors KeyboardLayout.LETTER_ROWS plus the leading controls that
// MSIMEInputService builds beneath them.

export function touchKeyboardSkinOption(skin: TouchKeyboardSkin): TouchKeyboardSkinOption {
  const id = skin === "custom" ? "forest" : skin;
  return touchKeyboardSkinOptions.find((option) => option.id === id) ?? touchKeyboardSkinOptions[0];
}

export function touchKeyboardSkinPalette(
  theme: "dark" | "light",
  skin: TouchKeyboardSkin,
  customDesign?: TouchKeyboardSkinDesign,
): TouchKeyboardSkinPalette {
  const custom = skin === "custom" && customDesign ? customDesign : undefined;
  if (!custom) return touchKeyboardSkinOption(skin)[theme];
  return {
    background: skinColor(custom.background),
    key: skinColor(custom.keyBackground),
    foreground: skinColor(custom.keyForeground),
    accent: skinColor(custom.accent),
    action: skinColor(custom.actionBackground),
  };
}

function Pattern({
  id,
  pattern,
  accent,
  opacity = 0.15,
}: {
  id: string;
  pattern: number;
  accent: string;
  opacity?: number;
}) {
  if (pattern === 0) return null;
  if (pattern === 1)
    return (
      <pattern id={id} width="16" height="16" patternUnits="userSpaceOnUse">
        <circle cx="8.75" cy="8.75" r=".75" fill={accent} fillOpacity={opacity} />
      </pattern>
    );
  if (pattern === 2)
    return (
      <pattern id={id} width="20" height="20" patternUnits="userSpaceOnUse">
        <path
          d="M0 0H20M0 0V20"
          fill="none"
          stroke={accent}
          strokeOpacity={opacity}
          strokeWidth=".5"
        />
      </pattern>
    );
  return (
    <pattern id={id} width="48" height="48" patternUnits="userSpaceOnUse">
      <path
        d="M-12 42C4 0 27 65 60 1M-12 66C4 24 27 89 60 25"
        fill="none"
        stroke={accent}
        strokeOpacity={opacity}
        strokeWidth="2"
      />
    </pattern>
  );
}

export function ScreenKeyboardPreview({
  theme,
  skin = "forest",
  compact = false,
  customDesign,
  keySpacingTenths = 60,
  rowSpacingTenths = 70,
  heightAdjustment = 0,
  layout = "desktop",
}: {
  theme: "dark" | "light";
  skin?: TouchKeyboardSkin;
  compact?: boolean;
  customDesign?: TouchKeyboardSkinDesign;
  keySpacingTenths?: number;
  rowSpacingTenths?: number;
  heightAdjustment?: number;
  layout?: "desktop" | "touch";
}) {
  const touch = layout === "touch";
  const layoutRows = touch ? touchKeyboardRows : desktopKeyboardRows;
  const custom = skin === "custom" && customDesign ? customDesign : undefined;
  const option = touchKeyboardSkinOption(skin);
  const palette = touchKeyboardSkinPalette(theme, skin, customDesign);
  const cornerRadius = custom?.cornerRadius ?? option.cornerRadius;
  const borderWidth = custom?.borderWidth ?? option.borderWidth;
  const shadowOpacity = custom?.shadow ?? option.shadowOpacity;
  const shadowRadius = custom ? 2 : option.shadowRadius;
  const shadowOffset = custom ? 1 : option.shadowOffset;
  const monospaced = custom?.monospaced ?? option.monospaced;
  const pattern = custom?.pattern ?? option.pattern;
  const keyShape = custom?.keyShape ?? "rounded";
  const keyMaterial = custom?.keyMaterial ?? "flat";
  const keyOpacity = custom?.keyOpacity ?? 1;
  const actionForeground = custom ? skinColor(readableSkinText(custom.actionBackground)) : "#fff";
  const unique = useId().replaceAll(":", "");
  const patternId = `touch-skin-pattern-${unique}`;
  const shadowId = `touch-skin-shadow-${unique}`;
  const backgroundId = `touch-skin-background-${unique}`;
  const keyMaterialId = `touch-skin-key-material-${unique}`;
  const actionMaterialId = `touch-skin-action-material-${unique}`;
  const keySpacing = Math.min(6, Math.max(3, keySpacingTenths / 10));
  const rowSpacing = Math.min(10, Math.max(4, rowSpacingTenths / 10));
  const canvasHeight = 400 + Math.min(48, Math.max(-12, heightAdjustment));
  // Keep the default artwork byte-for-byte equivalent while making the
  // non-default geometry visibly track the iOS keyboard settings sliders.
  const keyGap = 4 + keySpacing - 6;
  const rowGap = 4 + rowSpacing - 7;
  const height = (canvasHeight - 28 - 7 - rowGap * (layoutRows.length - 1)) / layoutRows.length;
  return (
    <svg
      className={`screen-keyboard-artwork${compact ? " compact" : ""}`}
      data-preview-theme={theme}
      data-preview-skin={skin}
      data-key-shape={keyShape}
      data-key-material={keyMaterial}
      data-key-spacing={keySpacing.toFixed(1)}
      data-row-spacing={rowSpacing.toFixed(1)}
      data-keyboard-height={canvasHeight}
      viewBox={`0 0 1100 ${canvasHeight}`}
      role={compact ? undefined : "img"}
      aria-hidden={compact || undefined}
      aria-label={compact ? undefined : "屏幕键盘完整布局预览"}
      style={{
        fontFamily: monospaced ? "ui-monospace, SFMono-Regular, Consolas, monospace" : undefined,
      }}
    >
      <defs>
        <Pattern
          id={patternId}
          pattern={pattern}
          accent={palette.accent}
          opacity={custom?.patternOpacity ?? 0.15}
        />
        {shadowOpacity > 0 && (
          <filter id={shadowId} x="-20%" y="-20%" width="140%" height="150%">
            <feDropShadow
              dx="0"
              dy={shadowOffset}
              stdDeviation={shadowRadius}
              floodOpacity={shadowOpacity}
            />
          </filter>
        )}
        {custom?.gradientEnd !== undefined && (
          <linearGradient
            id={backgroundId}
            x2={custom.gradientHorizontal ? "1" : "0"}
            y2={custom.gradientHorizontal ? "0" : "1"}
          >
            <stop stopColor={palette.background} />
            <stop offset="1" stopColor={skinColor(custom.gradientEnd)} />
          </linearGradient>
        )}
        {(keyMaterial === "glass" || keyMaterial === "raised") && (
          <>
            <linearGradient id={keyMaterialId} x2="0" y2="1">
              <stop stopColor="#fff" stopOpacity={keyMaterial === "glass" ? 0.24 : 0.13} />
              <stop offset=".48" stopColor={palette.key} stopOpacity={keyOpacity} />
              <stop
                offset="1"
                stopColor="#000"
                stopOpacity={keyMaterial === "glass" ? 0.03 : 0.1}
              />
            </linearGradient>
            <linearGradient id={actionMaterialId} x2="0" y2="1">
              <stop stopColor="#fff" stopOpacity={keyMaterial === "glass" ? 0.24 : 0.13} />
              <stop offset=".48" stopColor={palette.action} />
              <stop
                offset="1"
                stopColor="#000"
                stopOpacity={keyMaterial === "glass" ? 0.03 : 0.1}
              />
            </linearGradient>
          </>
        )}
      </defs>
      <rect
        width="1100"
        height={canvasHeight}
        rx="8"
        fill={custom?.gradientEnd === undefined ? palette.background : `url(#${backgroundId})`}
      />
      {custom?.photo && (
        <>
          <image
            href={`data:image/jpeg;base64,${custom.photo}`}
            width="1100"
            height={canvasHeight}
            preserveAspectRatio={`${(custom.photoPosition ?? 0.5) < 0.34 ? "xMinYMin" : (custom.photoPosition ?? 0.5) > 0.66 ? "xMaxYMax" : "xMidYMid"} slice`}
          />
          <rect
            width="1100"
            height={canvasHeight}
            fill="#000"
            fillOpacity={custom.photoShade ?? 0.25}
          />
        </>
      )}
      {pattern !== 0 && (
        <rect width="1100" height={canvasHeight} rx="8" fill={`url(#${patternId})`} />
      )}
      <text x="10" y="14" dominantBaseline="middle" fontSize="12" fill={palette.accent}>
        {touch ? "水杉 IME" : "Touch keyboard"}
      </text>
      {/* The close glyph belongs to the desktop panel, which floats in a window the user can dismiss.
          A phone's keyboard is dismissed by the system, so drawing an X there promises nothing. */}
      {!touch && (
        <path
          d="M1075 9l10 10m0-10l-10 10"
          fill="none"
          stroke={palette.foreground}
          strokeWidth="2"
        />
      )}
      {layoutRows.map((row, rowIndex) => {
        const available = 1100 - 14 - keyGap * (row.length - 1);
        const total = row.reduce((sum, item) => sum + item.weight, 0);
        let x = 7;
        const y = 28 + rowIndex * (height + rowGap);
        return (
          <g data-keyboard-row={rowIndex} key={rowIndex}>
            {row.map((item, index) => {
              const width = (available * item.weight) / total;
              const left = x;
              x += width + keyGap;
              // An unlabelled entry is the half-key inset that centres the home row, not a key the
              // user can press: it takes up its width and draws nothing.
              if (!item.label) return null;
              const action = actionKeyboardLabels.has(item.label);
              const path = keyboardKeyPath(
                left,
                y,
                width,
                height - (keyMaterial === "raised" ? 3 : 0),
                keyShape,
                cornerRadius,
              );
              const depthPath = keyboardKeyPath(
                left,
                y + 3,
                width,
                height - 3,
                keyShape,
                cornerRadius,
              );
              const fill =
                keyMaterial === "glass" || keyMaterial === "raised"
                  ? `url(#${action ? actionMaterialId : keyMaterialId})`
                  : action
                    ? palette.action
                    : palette.key;
              return (
                <g
                  data-keyboard-key={item.label}
                  key={index}
                  filter={shadowOpacity > 0 ? `url(#${shadowId})` : undefined}
                >
                  {keyMaterial === "raised" && (
                    <path
                      d={depthPath}
                      fill={action ? palette.action : palette.key}
                      opacity=".72"
                    />
                  )}
                  <path
                    d={path}
                    fill={fill}
                    fillOpacity={action ? 1 : keyOpacity}
                    stroke={
                      borderWidth
                        ? custom?.customBorderColor === undefined
                          ? palette.accent
                          : skinColor(custom.customBorderColor)
                        : "none"
                    }
                    strokeOpacity={custom ? 1 : skin === "midnight" ? 0.65 : 0.28}
                    strokeWidth={borderWidth}
                  />
                  {keyMaterial === "paper" && (
                    <path
                      d={`${path}M${left + 3} ${y + 10}H${left + width - 3}M${left + 3} ${y + 18}H${left + width - 3}M${left + 3} ${y + 26}H${left + width - 3}`}
                      fill="none"
                      stroke="#000"
                      strokeOpacity=".08"
                      strokeWidth=".5"
                    />
                  )}
                  <text
                    x={left + width / 2}
                    y={y + height / 2}
                    textAnchor="middle"
                    dominantBaseline="middle"
                    fontSize={item.label.length === 1 ? 15 : 12}
                    fill={action ? actionForeground : palette.foreground}
                  >
                    {item.label}
                  </text>
                </g>
              );
            })}
          </g>
        );
      })}
    </svg>
  );
}
