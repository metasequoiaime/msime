import {
  DecorationCandidatePolicy as Overlay,
  DecorationCandidateStyle,
  DecorationWordCandidate,
} from "../entry/src/main/ets/keyboard/candidate/DecorationCandidatePolicy";
import {
  CandidateSkinCatalogPolicy as Catalog,
  CandidateSkinPackage,
} from "../entry/src/main/ets/keyboard/candidate/CandidateSkinCatalogPolicy";

let assertions = 0;
function check(value: boolean, message: string): void {
  assertions++;
  if (!value) throw new Error(message);
}
const style: DecorationCandidateStyle = {
  index: 1,
  fontSizeDip: 20,
  minFontSizeDip: 8,
  maxWidthDip: 0,
  resolvedColor: "#12345680",
  alignH: "center",
  alignV: "center",
  offsetXDip: 0,
  offsetYDip: 0,
  stripBadge: true,
};
const page: DecorationWordCandidate[] = [
  { text: "合成(原文)", badge: "云", highlighted: false },
  { text: "测试", badge: "AI", highlighted: true },
];
check(Overlay.word(style, page) === "合成(原文)", "不删除原文的括号");
check(Overlay.word({ ...style, stripBadge: false }, page) === "合成(原文)云", "标注单独拼接");
check(Overlay.word({ ...style, index: 0 }, page) === "测试", "零跟随当前页高亮");
check(Overlay.word({ ...style, index: 2 }, page) === "测试", "从一开始编号");
check(Overlay.word({ ...style, index: 3 }, page) === "", "页外编号清理文字");
check(
  Overlay.word(
    { ...style, index: 0 },
    page.map((candidate) => ({ ...candidate, highlighted: false })),
  ) === "",
  "没有高亮不猜测第一项",
);
const measure = (text: string, fontSize: number) => ({
  width: [...text].length * fontSize,
  height: fontSize,
});
const fitted = Overlay.frame(style, page, 60, 40, measure);
check(
  fitted !== null && fitted.fontSize === 10 && fitted.width === 60 && fitted.y === 15,
  "按实际宽度缩小并居中",
);
check(fitted?.color === "#80123456", "颜色只转换一次为 ArkUI 的 ARGB");
const overflow = Overlay.frame({ ...style, maxWidthDip: 1 }, page, 10, 10, measure);
check(
  overflow?.fontSize === 8 && overflow.width === 48 && overflow.x === -19,
  "最小字号仍超框时保留整行",
);
check(
  Overlay.frame({ ...style, minFontSizeDip: 40 }, page, 10, 10, measure)?.fontSize === 20,
  "最小字号比初始字号大时不放大",
);
for (const alignH of ["left", "center", "right"]) {
  for (const alignV of ["top", "center", "bottom"]) {
    const frame = Overlay.frame(
      { ...style, index: 2, alignH, alignV, offsetXDip: -3, offsetYDip: 4 },
      page,
      100,
      80,
      measure,
    );
    check(
      frame?.x === -3 + (100 - 40) * (alignH === "left" ? 0 : alignH === "center" ? 0.5 : 1) &&
        frame.y === 4 + (80 - 20) * (alignV === "top" ? 0 : alignV === "center" ? 0.5 : 1),
      "九种对齐与有符号偏移",
    );
  }
}
for (const invalid of [
  null,
  { ...style, index: 0.5 },
  { ...style, fontSizeDip: 0 },
  { ...style, offsetXDip: NaN },
  { ...style, minFontSizeDip: 201 },
  { ...style, resolvedColor: "red" },
  { ...style, alignH: "unsafe" },
]) {
  check(Overlay.frame(invalid, page, 100, 80, measure) === null, "拒绝无效声明");
}
check(Overlay.frame(style, page, 0, 80, measure) === null, "零尺寸不绘制");
check(
  Overlay.frame(style, page, 100, 80, () => ({ width: NaN, height: 10 })) === null,
  "无效测量不传播",
);
const sample: CandidateSkinPackage = {
  id: "synthetic",
  base: "system",
  layouts: ["vertical"],
  themes: ["light"],
  minWidthDip: 200,
  decorationTopDip: 40,
  decorationWidthDip: 100,
  decorationImage: "decoration.png",
  preview: null,
  decorationCandidate: style,
};
check(Catalog.decorationCandidate([sample], "synthetic") === style, "通过完整清单交付声明");
check(
  Catalog.decorationCandidate([{ ...sample, decorationImage: null }], "synthetic") === null,
  "没有图片不绘制孤立文字",
);
check(Catalog.decorationCandidate([sample], "missing") === null, "切换到其他皮肤清理文字");
console.log(`decoration-candidate: ${assertions} assertions passed`);
