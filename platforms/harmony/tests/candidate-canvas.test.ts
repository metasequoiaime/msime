import {
  CandidateCanvasPolicy as Canvas,
  CandidateCanvasBorder,
  CandidateCanvasDecoration,
  CandidateCanvasSpec,
} from "../entry/src/main/ets/keyboard/candidate/CandidateCanvasPolicy";

let assertions = 0;
function check(value: boolean, message: string): void {
  assertions++;
  if (!value) throw new Error(message);
}
function equal(actual: Object, expected: Object, message: string): void {
  check(JSON.stringify(actual) === JSON.stringify(expected), message);
}

const zero = Canvas.ZERO;
equal(Canvas.sides([2], zero), { top: 2, right: 2, bottom: 2, left: 2 }, "一个值展开四边");
equal(Canvas.sides([2, 4], zero), { top: 2, right: 4, bottom: 2, left: 4 }, "两个值展开纵横");
equal(
  Canvas.sides([1, 2, 3, 4], zero),
  { top: 1, right: 2, bottom: 3, left: 4 },
  "四个值按 CSS 顺序",
);
equal(Canvas.sides([NaN], zero), zero, "无效数值不进入 Canvas");
equal(Canvas.cuts(6, 8, 4, 2), [2, 6, 6, 8], "两边超宽时等比夹紧");
const border: CandidateCanvasBorder = {
  source: "panel.png",
  sliceDip: [9],
  stretchDip: [2, 3],
  widthDip: [4, 6],
  fill: true,
};
equal(Canvas.slice(border), { top: 2, right: 3, bottom: 2, left: 3 }, "stretchDip 优先");
const patches = Canvas.patches(border, 20, 10, { x: 10, y: 20, width: 100, height: 50 }, 1);
check(patches.length === 9, "绘制完整九宫格");
equal(
  patches[0],
  {
    source: { x: 0, y: 0, width: 3, height: 2 },
    destination: { x: 10, y: 20, width: 6, height: 4 },
  },
  "像素切片独立于 vp 边宽",
);
equal(
  patches[4],
  {
    source: { x: 3, y: 2, width: 14, height: 6 },
    destination: { x: 16, y: 24, width: 88, height: 42 },
  },
  "中间区域正确拉伸",
);
const hollow: CandidateCanvasBorder = { source: "panel.png", sliceDip: [2], fill: false };
check(
  Canvas.patches(hollow, 20, 10, { x: 0, y: 0, width: 100, height: 50 }, 1).length === 8,
  "fill=false 不绘制中心",
);
const spec: CandidateCanvasSpec = {
  canvas: true,
  borderImage: border,
  contentInsetsDip: { top: 20, right: 12, bottom: 20, left: 12 },
  outerInsetsDip: { top: 2, right: 4, bottom: 2, left: 4 },
};
const geometry = Canvas.geometry(spec, 320, 50);
check(geometry.scale === 0.5, "固定高度按最紧的轴等比缩放");
check(50 - geometry.content.top - geometry.content.bottom === 28, "保留 28vp 文字区");
equal(geometry.card, { x: 2, y: 1, width: 316, height: 48 }, "外留白与几何同尺度");
const lane = Canvas.geometry(spec, 320, 50, undefined, 34);
check(
  50 - lane.content.top - lane.content.bottom === 34,
  "手机候选条传入 34vp 候选通道时，留白缩到让出整条通道",
);
const reading = Canvas.geometry(spec, 320, 50, undefined, 50);
check(
  reading.scale === 0 && reading.content.top === 0 && reading.content.bottom === 0,
  "读音行也占位、内容要满 50vp 时留白缩到零，文字不压在面板外",
);
const decoration = (
  anchor: string,
  image: string = anchor,
  zIndex: number = 2,
): CandidateCanvasDecoration => ({
  image: image,
  anchor: anchor,
  offsetXDip: 2,
  offsetYDip: 3,
  widthDip: 20,
  heightDip: 10,
  zIndex: zIndex,
});
const anchors: string[] = [
  "top-left",
  "top-center",
  "top-right",
  "center-left",
  "center",
  "center-right",
  "bottom-left",
  "bottom-center",
  "bottom-right",
];
for (let index = 0; index < anchors.length; index++) {
  const rect = Canvas.decorationRect(
    decoration(anchors[index]),
    20,
    10,
    { x: 10, y: 20, width: 100, height: 60 },
    1,
  );
  check(
    rect.x === [12, 52, 92][index % 3] && rect.y === [23, 48, 73][Math.floor(index / 3)],
    `${anchors[index]} 九锚点定位`,
  );
}
const contained = Canvas.decorationRect(
  decoration("center"),
  10,
  10,
  { x: 0, y: 0, width: 100, height: 60 },
  1,
);
equal(contained, { x: 47, y: 28, width: 10, height: 10 }, "挂件居中 contain 保持纵横比");
equal(
  Canvas.decorations([
    decoration("center", "a", 2),
    decoration("center", "b", -1),
    decoration("center", "c", 2),
  ]).map((value: CandidateCanvasDecoration): string => value.image),
  ["b", "a", "c"],
  "同 zIndex 保持清单顺序",
);
check(Canvas.acceptsImage(200, 100, 120, Canvas.MAX_PIXELS), "接受预算内动画");
check(!Canvas.acceptsImage(2049, 10, 1, Canvas.MAX_PIXELS), "拒绝过大边长");
check(!Canvas.acceptsImage(2048, 2048, 2, Canvas.MAX_PIXELS), "拒绝累计像素超预算");
check(!Canvas.acceptsImage(20, 10, 121, Canvas.MAX_PIXELS), "拒绝超帧数");
check(!Canvas.acceptsImage(20, 10, 0, Canvas.MAX_PIXELS), "拒绝无帧图片");
equal(
  [0, 10, NaN, 11, 50].map((value: number): number => Canvas.delay(value)),
  [100, 100, 100, 11, 50],
  "归一化超短帧和非法帧时长",
);
check(
  Canvas.frame([50, 100], 49, 0) === 0 && Canvas.frame([50, 100], 50, 0) === 1,
  "帧时长边界切换",
);
check(Canvas.frame([50, 100], 150, 0) === 0, "无限循环回到首帧");
check(Canvas.frame([50, 100], 300, 2) === 1, "有限循环停在末帧");
console.log(`candidate-canvas: ${assertions} assertions passed`);

const gifHeader: number[] = [71, 73, 70, 56, 57, 97, 1, 0, 1, 0, 0, 0, 0];
const appExtension: number[] = [33, 255, 11, 78, 69, 84, 83, 67, 65, 80, 69, 50, 46, 48, 3, 1];
const loops = (contentType: string, bytes: number[]): number =>
  Canvas.loops(contentType, new Uint8Array(bytes));
check(loops("image/gif", gifHeader.concat([59])) === 1, "没有扩展的 GIF 只播放一轮");
check(
  loops("image/gif", gifHeader.concat(appExtension, [2, 0, 0, 59])) === 3,
  "GIF 扩展值二表示三轮",
);
check(
  loops("image/gif", gifHeader.concat(appExtension, [0, 0, 0, 59])) === 0,
  "GIF 扩展值零无限循环",
);
check(
  loops(
    "image/png",
    [
      137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 8, 97, 99, 84, 76, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0,
      0,
    ],
  ) === 3,
  "APNG 循环数已经是总轮数",
);
check(
  loops(
    "image/webp",
    [82, 73, 70, 70, 18, 0, 0, 0, 87, 69, 66, 80, 65, 78, 73, 77, 6, 0, 0, 0, 0, 0, 0, 0, 3, 0],
  ) === 3,
  "WebP ANIM 循环数按小端读取",
);
console.log(`candidate-canvas: ${assertions} total assertions passed`);
check(Canvas.extraWidth(spec) === 32, "桌面窗宽包含完整内外留白");
check(Canvas.extraHeight(spec, false) === 44, "隐藏预编辑时不额外增加行距");
const spaced: CandidateCanvasSpec = { canvas: true, borderImage: border, lineGapDip: 7 };
check(
  Canvas.extraHeight(spaced, true) === 15 && Canvas.extraHeight(spaced, false) === 8,
  "行距只在预编辑行存在时参与窗口高度",
);
console.log(`candidate-canvas: ${assertions} final assertions passed`);
