import { CandidateLayoutPolicy as Layout } from "../entry/src/main/ets/keyboard/candidate/CandidateSkinPolicy";
import { GlobalTheme } from "../entry/src/main/ets/keyboard/skin/GlobalTheme";
import { KeyboardMetrics } from "../entry/src/main/ets/keyboard/KeyboardMetrics";

function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}
const source = {
  cornerRadiusDip: 0,
  itemCornerRadiusDip: 0,
  borderWidthDip: 0,
  fitContent: false,
  horizontal: { itemGap: { value: 0.3, em: true }, numberScale: 0 },
  vertical: { itemGap: { value: 5, em: false }, numberScale: 1 },
};
const theme = GlobalTheme.parseResolved(
  JSON.stringify({ ok: true, value: { id: "custom", source: "custom", candidate_layout: source } }),
);
const layout = theme?.candidate_layout;
check(layout !== null && layout !== undefined, "共享版式必须进入主题结果");
check(
  layout?.itemRadius === 0 && layout.borderWidth === 0 && layout.cornerRadius === 0,
  "显式零值不能变成原生默认",
);
check(Layout.dip(layout?.horizontal.itemGap, 19, 99) === 6, "em 乘实际字号后取整");
check(Layout.dip(layout?.vertical.itemGap, 19, 99) === 5, "竖排保持 DIP");
check(Layout.numberSize(layout?.horizontal, 19, 14) === 0, "序号缩放零值仍表示隐藏");
check(
  Layout.gap(layout, false, 19) === 12 && Layout.gap(layout, true, 19) === 5,
  "横排保留非紧凑间距，竖排不附加",
);
check(
  KeyboardMetrics.candidateHeightVp("vertical", 3, true, 0, 0, 19, 16, 5) ===
    KeyboardMetrics.candidateHeightVp("vertical", 3, true, 0, 0, 19, 16) + 10,
  "窗口预留全部竖排行间距，避免裁切末行",
);
check(
  Layout.chipWidth(
    {
      ...layout!.horizontal,
      itemPaddingRight: { value: 0, em: false },
      itemMinWidth: { value: 80, em: false },
    },
    19,
    30,
    11,
  ) === 80,
  "零右内边距与候选最小宽度进入测量",
);
check(Layout.parse(undefined) === null, "旧响应不产生额外版式");
const invalid = Layout.parse({
  itemCornerRadiusDip: 999,
  geometry: { itemGap: { value: -1, em: false } },
});
check(invalid?.itemRadius === null && invalid.horizontal.itemGap === null, "无效尺寸使用原生回退");
console.log("HarmonyOS 候选版式：主题传递、横竖排、DIP/em、零值与回退通过");
