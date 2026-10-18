/** 水杉娘插画出现的场景。每个场景一张图，只用在低频页面（更新完成、空列表、连不上服务、下载中），不进入候选窗等高频输入区域。 */
export type MascotScene = "updated" | "empty" | "offline" | "downloading";

const mascotImages: Record<MascotScene, string> = {
  updated: new URL("../assets/mascot/updated.webp", import.meta.url).href,
  empty: new URL("../assets/mascot/empty.webp", import.meta.url).href,
  offline: new URL("../assets/mascot/offline.webp", import.meta.url).href,
  downloading: new URL("../assets/mascot/downloading.webp", import.meta.url).href,
};

export interface MascotIllustrationProps {
  scene: MascotScene;
  /** 显示边长（CSS 像素）。图源是 256px，最大按 128 用，保证 2 倍屏下仍清晰。 */
  size?: number;
  className?: string;
}

/** 水杉娘插画。纯装饰：旁边的文字已经说明了状态，所以对辅助技术隐藏，不替代任何文字。图源是 256px 的透明 WebP；HarmonyOS 设置页是单文件 HTML，会把它们内联成 base64，四张合计约 95KB。 */
export function MascotIllustration({ scene, size = 96, className }: MascotIllustrationProps) {
  return (
    <img
      src={mascotImages[scene]}
      alt=""
      aria-hidden="true"
      draggable={false}
      width={size}
      height={size}
      data-mascot={scene}
      className={`pointer-events-none select-none${className ? ` ${className}` : ""}`}
    />
  );
}
