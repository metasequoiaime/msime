import {
  ToolbarImagePolicy as Policy,
  ToolbarImageScene,
} from "../entry/src/main/ets/keyboard/ToolbarImagePolicy";
import {
  ToolbarButton,
  FloatingToolbarLayout,
} from "../entry/src/main/ets/keyboard/FloatingToolbarLayout";

let checks = 0;
function check(value: boolean, message: string): void {
  checks++;
  if (!value) throw new Error(message);
}
const scene: ToolbarImageScene = {
  key: "synthetic",
  resources: [],
  spec: {
    image: "bar.apng",
    imageWidthDip: 80,
    imageHeightDip: 40,
    buttonDefaults: { background: "button.png" },
    buttons: { status: { image: "fallback.webp", images: { cn: "cn.png", en: "bar.apng" } } },
  },
  layout: {
    widthDip: 110,
    heightDip: 60,
    buttons: [
      { key: "status", xDip: 90, yDip: 0, widthDip: 20, heightDip: 20, iconSizeDip: 16.4 },
      { key: "settings", xDip: 0, yDip: 40, widthDip: 20, heightDip: 20, iconSizeDip: 16.4 },
    ],
  },
};
const alpha = new Uint8Array([255, 0, 0, 255]);
const shape = Policy.mask(scene, 1.5, alpha, 4, 1);
check(shape !== null && shape.length === 90 && shape[0].length === 165, "物理尺寸与面板缩放一致");
check(shape![10][10] === 1 && shape![10][110] === 1, "背景命中使用两端动画帧的并集");
check(shape![10][60] === 0 && shape![50][140] === 0, "背景孔和背景外空白交还系统");
check(shape![10][140] === 1 && shape![70][5] === 1, "背景外按钮完整命中");
const fallback = Policy.mask(scene, 1.5, null, 0, 0);
check(fallback![10][60] === 1 && fallback![50][140] === 0, "解码失败只回退背景矩形");
check(Policy.mask(scene, 100, alpha, 4, 1) === null, "超大二维数组在分配前拒绝");
check(Policy.mask(scene, NaN, alpha, 4, 1) === null, "无效密度不能进入窗口接口");
check(
  Policy.paths(scene.spec).join("|") === "bar.apng|button.png|fallback.webp|cn.png",
  "所有状态图去重预载",
);
const state = FloatingToolbarLayout.idleState();
check(
  JSON.stringify(
    Policy.shown([ToolbarButton.INPUT_MODE, ToolbarButton.VOICE, ToolbarButton.SETTINGS], state),
  ) ===
    JSON.stringify([
      ["status", "cn"],
      ["settings", ""],
    ]),
  "只提交宿主启用且图片契约支持的按钮",
);
check(
  Policy.shown([ToolbarButton.INPUT_MODE], { ...state, capsLock: true, english: true })[0][1] ===
    "caps",
  "Caps Lock 优先",
);
check(
  Policy.shown([ToolbarButton.INPUT_MODE], { ...state, temporaryEnglish: true })[0][1] ===
    "en-candidate",
  "临时英文仍有独立状态图",
);
console.log(`图片工具栏 ${checks} 项断言通过`);
