import { dimension } from "./skin-preview-geometry";
import type { ExternalSkin, SkinInsets } from "./external-skins";

/** canvas 的一条边；切片、边宽、内容与外留白都用它。 */
export type SkinEdges = { top: number; right: number; bottom: number; left: number };

/** 九宫格面板：绘制明暗的包内图片、源像素切片、屏幕上四边的宽度和中心块是否填充。 */
export type SkinCanvasPanel = {
  image: string;
  slice: SkinEdges;
  width: SkinEdges;
  fill: boolean;
};

export type SkinCanvasAnchor =
  | "top-left"
  | "top-right"
  | "bottom-left"
  | "bottom-right"
  | "left"
  | "right"
  | "top"
  | "bottom"
  | "center";

/** 一个自由挂件：框按锚点贴卡片，再平移偏移，图片在框内等比缩放。 */
export type SkinCanvasDecoration = {
  image: string;
  anchor: SkinCanvasAnchor;
  offsetX: number;
  offsetY: number;
  width: number;
  height: number;
  zIndex: number;
};

/** 一个 canvas 包在设置页卡片里画出来的几何；`panel` 只描述清单几何，是否解码成功由图片读取层判断，失败时卡片保留可读回退底色。 */
export type SkinCanvasGeometry = {
  panel: SkinCanvasPanel | null;
  contentInsets: SkinEdges | null;
  outerInsets: SkinEdges | null;
  lineGap: number | null;
  below: SkinCanvasDecoration[];
  above: SkinCanvasDecoration[];
};

const anchors: SkinCanvasAnchor[] = [
  "top-left",
  "top-right",
  "bottom-left",
  "bottom-right",
  "left",
  "right",
  "top",
  "bottom",
  "center",
];

/**
 * CSS 四值简写的四边：1 个值四边相同，2 个值是「上下 左右」，4 个值是「上右下左」。其余个数或越界返回 `null`
 * （共享 catalog 只放行 1、2、4 个并已按同一上限校验，这里再挡一次清单外的数据）。
 */
export function canvasEdges(
  values: number[] | null | undefined,
  maximum: number,
): SkinEdges | null {
  if (!Array.isArray(values)) return null;
  if (
    !values.every(
      (value) =>
        typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= maximum,
    )
  )
    return null;
  const [a, b, c, d] = values;
  if (values.length === 1) return { top: a, right: a, bottom: a, left: a };
  if (values.length === 2) return { top: a, right: b, bottom: a, left: b };
  if (values.length === 4) return { top: a, right: b, bottom: c, left: d };
  return null;
}

function readInsets(value: SkinInsets | null | undefined, maximum: number): SkinEdges | null {
  if (!value) return null;
  const sides = [value.top, value.right, value.bottom, value.left];
  if (
    !sides.every(
      (side) => typeof side === "number" && Number.isFinite(side) && side >= 0 && side <= maximum,
    )
  )
    return null;
  return { top: value.top, right: value.right, bottom: value.bottom, left: value.left };
}

function signed(value: number | null | undefined, maximum: number, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= maximum
    ? value
    : fallback;
}

/**
 * 一个 canvas 包的卡片预览几何，取值规则与 Windows 的 `CandidateCanvas.h` 和网页引擎的 `canvasGeometry` 相同：
 * 浅色用 `lightSource`（没写就用 `source`），`stretchDip` 盖过 `sliceDip` 当切片，`widthDip` 缺省取切片；
 * 内容与外留白只在四边齐全且在界内时生效；挂件按 `zIndex` 分层，低于 1 的画在面板之下。
 *
 * 不是 canvas 包时返回 `null`，卡片与以前逐位相同。
 */
export function skinCanvasGeometry(
  skin: ExternalSkin,
  theme: "dark" | "light",
): SkinCanvasGeometry | null {
  if (skin.canvas !== true) return null;
  const raw = skin.borderImage;
  const slice = canvasEdges(raw?.stretchDip, 500) ?? canvasEdges(raw?.sliceDip, 500);
  const source = raw?.source ?? "";
  const image = theme === "light" ? (raw?.lightSource ?? source) : source;
  const panel =
    raw && image && slice
      ? {
          image,
          slice,
          width: canvasEdges(raw.widthDip, 500) ?? slice,
          fill: raw.fill !== false,
        }
      : null;
  const decorations = (skin.decorations ?? [])
    .slice(0, 32)
    .flatMap((decoration): SkinCanvasDecoration[] => {
      const width = dimension(decoration.widthDip, 2000);
      const height = dimension(decoration.heightDip, 2000);
      if (!decoration.image || width <= 0 || height <= 0) return [];
      const anchor = anchors.includes(decoration.anchor as SkinCanvasAnchor)
        ? (decoration.anchor as SkinCanvasAnchor)
        : "right";
      const zIndex = signed(decoration.zIndex, 1000, 2);
      return [
        {
          image: decoration.image,
          anchor,
          offsetX: signed(decoration.offsetXDip, 2000, 0),
          offsetY: signed(decoration.offsetYDip, 2000, 0),
          width,
          height,
          zIndex: Number.isInteger(zIndex) ? zIndex : 2,
        },
      ];
    })
    .sort((a, b) => a.zIndex - b.zIndex);
  const lineGap = skin.lineGapDip;
  return {
    panel,
    contentInsets: readInsets(skin.contentInsetsDip, 500),
    outerInsets: readInsets(skin.outerInsetsDip, 500),
    lineGap:
      typeof lineGap === "number" && Number.isFinite(lineGap) && lineGap >= 0 && lineGap <= 100
        ? lineGap
        : null,
    below: decorations.filter((decoration) => decoration.zIndex < 1),
    above: decorations.filter((decoration) => decoration.zIndex >= 1),
  };
}

/** 九宫格面板的浏览器绘制属性：切片取自源图像素，边宽按 DIP，四边拉伸（CSS `border-image` 的默认行为）。键用 React 认的 camelCase。 */
export function canvasPanelStyle(panel: SkinCanvasPanel, url: string): Record<string, string> {
  const { slice, width, fill } = panel;
  return {
    borderImageSource: `url("${url}")`,
    borderImageSlice: `${slice.top} ${slice.right} ${slice.bottom} ${slice.left}${fill ? " fill" : ""}`,
    borderImageWidth: `${width.top}px ${width.right}px ${width.bottom}px ${width.left}px`,
    borderImageRepeat: "stretch",
  };
}

/**
 * 一个自由挂件的框：水平贴左／右／居中，竖直贴上／下／居中，再平移偏移；居中的轴用 `transform` 把自己挪回去。
 * `anchor` 字符串里带 `left` / `right` / `top` / `bottom` 的词决定贴哪条边，`center`、`top`、`bottom`、`left`、`right`
 * 缺的那条轴居中（与 Windows 的 `candidate_canvas_decoration_box` 相同）。
 */
export function canvasDecorationStyle(decoration: SkinCanvasDecoration): Record<string, string> {
  const { anchor, offsetX, offsetY, width, height } = decoration;
  const style: Record<string, string> = { width: `${width}px`, height: `${height}px` };
  const transforms: string[] = [];
  if (anchor.includes("left")) style["left"] = `${offsetX}px`;
  else if (anchor.includes("right")) style["right"] = `${-offsetX}px`;
  else {
    style["left"] = `calc(50% + ${offsetX}px)`;
    transforms.push("translateX(-50%)");
  }
  if (anchor.includes("top")) style["top"] = `${offsetY}px`;
  else if (anchor.includes("bottom")) style["bottom"] = `${-offsetY}px`;
  else {
    style["top"] = `calc(50% + ${offsetY}px)`;
    transforms.push("translateY(-50%)");
  }
  if (transforms.length) style["transform"] = transforms.join(" ");
  return style;
}

/**
 * 预览表面上的 canvas 变量，候选标记里的选择器按继承读到它们：内容内边距每边不小于九宫格边宽
 * （没写内容内边距时是边宽加卡片自己的内边距），外留白照清单，拼音行距只在写了 `line_gap_dip` 时才覆盖。
 */
export function canvasVariables(canvas: SkinCanvasGeometry): Record<string, string> {
  const border = canvas.panel?.width ?? { top: 0, right: 0, bottom: 0, left: 0 };
  const variables: Record<string, string> = {
    "--msime-canvas-outer-top": `${canvas.outerInsets?.top ?? 0}px`,
    "--msime-canvas-outer-right": `${canvas.outerInsets?.right ?? 0}px`,
    "--msime-canvas-outer-bottom": `${canvas.outerInsets?.bottom ?? 0}px`,
    "--msime-canvas-outer-left": `${canvas.outerInsets?.left ?? 0}px`,
  };
  for (const side of ["top", "right", "bottom", "left"] as const) {
    const base = side === "top" || side === "bottom" ? "-y" : "-x";
    variables[`--msime-canvas-content-${side}`] = canvas.contentInsets
      ? `${Math.max(canvas.contentInsets[side], border[side])}px`
      : `calc(${border[side]}px + var(--msime-candidate-pad${base}, 0px))`;
  }
  if (canvas.lineGap !== null) variables["--msime-canvas-line-gap"] = `${canvas.lineGap}px`;
  return variables;
}
