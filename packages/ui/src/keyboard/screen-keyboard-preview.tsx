import { createContext, useContext, useId } from "react";
import { themeEntry, type GlobalTheme } from "../theme/global-theme";
import { clamp } from "../core/number";
import {
  readableSkinText,
  skinColor,
  type TouchKeyboardSkinDesign,
} from "./touch-keyboard-skin-design";
import { actionKeyboardLabels, desktopKeyboardRows, touchKeyboardRows } from "./keyboard-layouts";
import { keyboardKeyPath } from "./keyboard-shape";
import { msimeFramePath, msimeStrokePath } from "../core/brand-logo";

type Palette = {
  background: string;
  key: string;
  foreground: string;
  accent: string;
  action: string;
  actionForeground: string;
};
/** How a global theme draws a touch keyboard when no custom design applies. */
export type KeyboardThemeLook = {
  palette: Palette;
  cornerRadius: number;
  borderWidth: number;
  shadowOpacity: number;
  shadowRadius: number;
  shadowOffset: number;
  monospaced: boolean;
  pattern: 0 | 1 | 2 | 3;
};

// `system` is the platform's own keyboard, which the page cannot read; it previews the design's iOS system keyboard tokens (dc.html `kbBg` / `keyBg` / `keySpec` / `keyFg` and the brand accent).
const systemKeyboard: Record<"dark" | "light", Palette> = {
  light: {
    background: "#D1D4DB",
    key: "#FFFFFF",
    foreground: "#000000",
    accent: "#2C7A4B",
    action: "#ABB0BB",
    actionForeground: "#000000",
  },
  dark: {
    background: "#2B2B2D",
    key: "#6B6B6E",
    foreground: "#FFFFFF",
    accent: "#5FBF84",
    action: "#464648",
    actionForeground: "#FFFFFF",
  },
};

// `native`（原生，只在 iOS 提供）照 iOS 26 起的系统键盘画，与 iOS 的 `SystemKeyboardTokens` 同值：底板 `#E2E3E8` / `#222223`，字母键和功能键同为 `#FFFFFF` / `#464646`，强调色是系统蓝的增强对比度值 `#0040DD` / `#409CFF`，圆角 6，不跟季节。
const nativeKeyboard: Record<"dark" | "light", Palette> = {
  light: {
    background: "#E2E3E8",
    key: "#FFFFFF",
    foreground: "#000000",
    accent: "#0040DD",
    action: "#FFFFFF",
    actionForeground: "#000000",
  },
  dark: {
    background: "#222223",
    key: "#464646",
    foreground: "#FFFFFF",
    accent: "#409CFF",
    action: "#464646",
    actionForeground: "#FFFFFF",
  },
};

/** 全局主题的键盘外观：内置主题用目录里的调色板（功能键画 `function_key`）；`native` 用系统键盘的取值；`system`、没有设计的 `custom` 和不认识的 id 用系统预览。 */
export function keyboardThemeLook(skin: string, theme: "dark" | "light"): KeyboardThemeLook {
  const entry = themeEntry(skin);
  const keyboard = entry.keyboard;
  const native = entry.id === "native";
  return {
    palette: keyboard
      ? {
          background: keyboard.background,
          key: keyboard.key,
          foreground: keyboard.text,
          accent: keyboard.accent,
          action: keyboard.function_key,
          actionForeground: keyboard.text,
        }
      : native
        ? nativeKeyboard[theme]
        : systemKeyboard[theme],
    cornerRadius: native ? 6 : 8,
    borderWidth: 0,
    shadowOpacity: 0,
    shadowRadius: 3,
    shadowOffset: 2,
    monospaced: false,
    pattern: 0,
  };
}

// Layout source: MSIME-Windows@04a8df56f86312474a069f4335a1b58da7afaa9e,
// server/src/keyboard-panel/KeyboardPanel.cpp (GPL-3.0). This preview has no input actions.
// The touch hosts put three letter rows above a control strip, so the desktop artwork above is the
// wrong picture of them: it promises a number row, Tab, Caps Lock and Win keys that a phone keyboard
// simply does not have. Mirrors KeyboardLayout.LETTER_ROWS plus the leading controls that
// MSIMEInputService builds beneath them.

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

// 手机皮肤网格的缩略图画布：设计里的 MiniKb，一个 390 × 292 的触屏键盘，上方是 50 单位高的工具栏，下面四行各 43 单位高、行距 11，键距 6，左右各内缩 3。
const thumbnailWidth = 390;
const thumbnailHeight = 292;
const thumbnailToolbarHeight = 50;
const thumbnailInset = 3;
// 键盘工具栏上水杉标志之后的图标，以绘制它们的 24 单位方框为坐标：表情、常用语、剪贴板、皮肤和键盘，最后是收起箭头。
const thumbnailToolbarGlyphs = [
  "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20zM8 14s1.5 2 4 2 4-2 4-2M9 9h.01M15 9h.01",
  "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2zM8 9h8M8 13h5",
  "M9 2h6a1 1 0 0 1 1 1v2a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1zM16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2M12 11h4M12 16h4M8 11h.01M8 16h.01",
  "M12 22a1 1 0 0 1 0-20 10 9 0 0 1 10 9 5 5 0 0 1-5 5h-2.25a1.75 1.75 0 0 0-1.4 2.8l.3.4a1.75 1.75 0 0 1-1.4 2.8zM13.5 6.5h.01M17.5 10.5h.01M6.5 12.5h.01M8.5 7.5h.01",
  "M10 8h.01M12 12h.01M14 8h.01M16 12h.01M18 8h.01M6 8h.01M7 16h10M8 12h.01M4 4h16a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z",
  "m6 9 6 6 6-6",
];

/** 缩略图的工具栏条：水杉标志用皮肤的强调色，其后的工具栏图标用它的按键文字色，每个占一个等宽列。 */
function ThumbnailToolbar({ accent, foreground }: { accent: string; foreground: string }) {
  const column = (thumbnailWidth - thumbnailInset * 2) / (thumbnailToolbarGlyphs.length + 1);
  const centre = (index: number) => thumbnailInset + column * (index + 0.5);
  const middle = thumbnailToolbarHeight / 2;
  return (
    <g data-keyboard-toolbar="">
      <svg x={centre(0) - 13} y={middle - 13} width="26" height="26" viewBox="0 0 116 132">
        <path d={msimeFramePath} fill={accent} />
        <path
          d={msimeStrokePath}
          fill="none"
          stroke="#FFFFFF"
          strokeWidth="9"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
      {thumbnailToolbarGlyphs.map((glyph, index) => (
        <path
          key={index}
          d={glyph}
          transform={`translate(${centre(index + 1) - 11} ${middle - 11}) scale(${22 / 24})`}
          fill="none"
          stroke={foreground}
          strokeWidth="1.7"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      ))}
    </g>
  );
}

/** 宿主自己的屏幕键盘画成哪种：Android、iOS 和 HarmonyOS 的输入法在任何形态下都是触屏键盘（三排字母加一排功能键），桌面宿主是 Windows 式的屏幕键盘。设置页在根部按宿主提供（`screenKeyboardLayoutFor`），没写 `layout` 的预览就画这一种，手机上因此不会出现 Win、Alt、Caps Lock。 */
export const ScreenKeyboardLayoutContext = createContext<"desktop" | "touch">("desktop");

/** `platform` 宿主的屏幕键盘布局，供 `ScreenKeyboardLayoutContext` 使用。 */
export function screenKeyboardLayoutFor(platform: string | undefined): "desktop" | "touch" {
  return platform === "android" || platform === "ios" || platform === "harmony"
    ? "touch"
    : "desktop";
}

/**
 * 用皮肤配色画的键盘。`compact` 是小型装饰卡片的插图；`thumbnail` 是手机皮肤网格的卡片插图，即带工具栏的触屏键盘，画在设计的 390 × 292 画布上，填满容器宽度（圆角和描边由容器负责）。两者都对辅助技术隐藏，因为外面的卡片已带有名称。
 */
export function ScreenKeyboardPreview({
  theme,
  skin = "system",
  compact = false,
  thumbnail = false,
  customDesign,
  keySpacingTenths = 60,
  rowSpacingTenths = 70,
  heightAdjustment = 0,
  layout,
}: {
  theme: "dark" | "light";
  skin?: GlobalTheme;
  compact?: boolean;
  thumbnail?: boolean;
  customDesign?: TouchKeyboardSkinDesign;
  keySpacingTenths?: number;
  rowSpacingTenths?: number;
  heightAdjustment?: number;
  /** 省略时取 `ScreenKeyboardLayoutContext`，即宿主自己的键盘。 */
  layout?: "desktop" | "touch";
}) {
  const hostLayout = useContext(ScreenKeyboardLayoutContext);
  const touch = thumbnail || (layout ?? hostLayout) === "touch";
  const decorative = compact || thumbnail;
  const layoutRows = touch ? touchKeyboardRows : desktopKeyboardRows;
  const custom = skin === "custom" && customDesign ? customDesign : undefined;
  const option = keyboardThemeLook(skin, theme);
  const palette = custom
    ? {
        background: skinColor(custom.background),
        key: skinColor(custom.keyBackground),
        foreground: skinColor(custom.keyForeground),
        accent: skinColor(custom.accent),
        action: skinColor(custom.actionBackground),
        actionForeground: skinColor(readableSkinText(custom.actionBackground)),
      }
    : option.palette;
  const cornerRadius = custom?.cornerRadius ?? (thumbnail ? 5 : option.cornerRadius);
  const borderWidth = custom?.borderWidth ?? option.borderWidth;
  const shadowOpacity = custom?.shadow ?? option.shadowOpacity;
  const shadowRadius = custom ? 2 : option.shadowRadius;
  const shadowOffset = custom ? 1 : option.shadowOffset;
  const monospaced = custom?.monospaced ?? option.monospaced;
  const pattern = custom?.pattern ?? option.pattern;
  const keyShape = custom?.keyShape ?? "rounded";
  const keyMaterial = custom?.keyMaterial ?? "flat";
  const keyOpacity = custom?.keyOpacity ?? 1;
  const unique = useId().replaceAll(":", "");
  const patternId = `touch-skin-pattern-${unique}`;
  const shadowId = `touch-skin-shadow-${unique}`;
  const backgroundId = `touch-skin-background-${unique}`;
  const keyMaterialId = `touch-skin-key-material-${unique}`;
  const actionMaterialId = `touch-skin-action-material-${unique}`;
  const keySpacing = clamp(keySpacingTenths / 10, 3, 6);
  const rowSpacing = clamp(rowSpacingTenths / 10, 4, 10);
  // 触屏键盘按手机竖屏的比例画：桌面画布 1100 宽时，每排十个键被拉成又扁又宽的长条，不像手机上的键盘；700 宽时单键约为宽 3 高 4 的竖长方形，与手机上的键接近。桌面键盘仍是 1100。
  const canvasWidth = thumbnail ? thumbnailWidth : touch ? 700 : 1100;
  const canvasHeight = thumbnail ? thumbnailHeight : 400 + clamp(heightAdjustment, -12, 48);
  const inset = thumbnail ? thumbnailInset : 7;
  const rowsTop = thumbnail ? thumbnailToolbarHeight + 8 : 28;
  // 默认插图保持逐字节等价，非默认的几何则明显跟随 iOS 键盘设置里的滑块。缩略图保留设计的固定几何，因为它展示的是皮肤而不是用户的键距。
  const keyGap = thumbnail ? 6 : 4 + keySpacing - 6;
  const rowGap = thumbnail ? 11 : 4 + rowSpacing - 7;
  const height = thumbnail
    ? 43
    : (canvasHeight - 28 - 7 - rowGap * (layoutRows.length - 1)) / layoutRows.length;
  const backgroundRadius = thumbnail ? 0 : 8;
  return (
    <svg
      className={
        thumbnail ? "block h-auto w-full" : `screen-keyboard-artwork${compact ? " compact" : ""}`
      }
      data-preview-theme={theme}
      data-preview-skin={skin}
      data-key-shape={keyShape}
      data-key-material={keyMaterial}
      data-key-spacing={keySpacing.toFixed(1)}
      data-row-spacing={rowSpacing.toFixed(1)}
      data-keyboard-height={canvasHeight}
      viewBox={`0 0 ${canvasWidth} ${canvasHeight}`}
      role={decorative ? undefined : "img"}
      aria-hidden={decorative || undefined}
      aria-label={decorative ? undefined : "屏幕键盘完整布局预览"}
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
        width={canvasWidth}
        height={canvasHeight}
        rx={backgroundRadius}
        fill={custom?.gradientEnd === undefined ? palette.background : `url(#${backgroundId})`}
      />
      {custom?.photo && (
        <>
          <image
            href={`data:image/jpeg;base64,${custom.photo}`}
            width={canvasWidth}
            height={canvasHeight}
            preserveAspectRatio={`${(custom.photoPosition ?? 0.5) < 0.34 ? "xMinYMin" : (custom.photoPosition ?? 0.5) > 0.66 ? "xMaxYMax" : "xMidYMid"} slice`}
          />
          <rect
            width={canvasWidth}
            height={canvasHeight}
            fill="#000"
            fillOpacity={custom.photoShade ?? 0.25}
          />
        </>
      )}
      {pattern !== 0 && (
        <rect
          width={canvasWidth}
          height={canvasHeight}
          rx={backgroundRadius}
          fill={`url(#${patternId})`}
        />
      )}
      {thumbnail ? (
        <ThumbnailToolbar accent={palette.accent} foreground={palette.foreground} />
      ) : (
        <text x="10" y="14" dominantBaseline="middle" fontSize="12" fill={palette.accent}>
          {touch ? "水杉 IME" : "Touch keyboard"}
        </text>
      )}
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
        const available = canvasWidth - inset * 2 - keyGap * (row.length - 1);
        const total = row.reduce((sum, item) => sum + item.weight, 0);
        let x = inset;
        const y = rowsTop + rowIndex * (height + rowGap);
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
                    strokeOpacity={custom ? 1 : 0.28}
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
                    fontSize={item.label.length === 1 ? (thumbnail ? 22 : 15) : thumbnail ? 15 : 12}
                    fill={action ? palette.actionForeground : palette.foreground}
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
