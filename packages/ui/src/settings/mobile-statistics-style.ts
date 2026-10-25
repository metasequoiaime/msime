import { accentMix } from "./typing-chart";

/** 绘制由摘要驱动的统计页的两种 HarmonyOS 外观：手机，以及内容相同、卡片圆角更小的 2-in-1。 */
export type StatisticsLook = "harmony" | "hm2";

export const subText = "text-[color:var(--p-sub)]";
/** 设计稿的 `stSegBg`：浅色模式下是卡片上一层淡淡的强调色，深色模式下是半透明的白色。 */
export const segmentTrack =
  "bg-[rgba(255,255,255,0.08)] light-theme:bg-[color-mix(in_srgb,var(--accent-color)_9%,var(--p-group-bg))]";
export const segmentOn =
  "bg-[#636366] font-semibold text-[color:var(--p-text)] light-theme:bg-white";
/** 非高亮的柱子：浅色模式下是卡片上 40% 的强调色，深色模式下是 45%。设计稿原是 `aM(dark ? 24 : 18)`，在手机上太淡，非今日的柱子几乎看不出高低；三端（这里、iOS 的 `StatisticsStyle.mutedBar`、Android 的 `ms_*_stat_bar`）用同一组比例。 */
export const quietBar =
  "bg-[color-mix(in_srgb,var(--accent-color)_45%,var(--p-group-bg))] light-theme:bg-[color-mix(in_srgb,var(--accent-color)_40%,var(--p-group-bg))]";

/** 占比图的颜色，按部分的先后排列；第四个之后的部分沿用最后一种。 */
const shareShades = [100, 52, 28, 15];
export const shade = (percentage: number) =>
  percentage >= 100 ? "var(--accent-color)" : accentMix(percentage);
export const shareColour = (index: number) =>
  shade(shareShades[Math.min(index, shareShades.length - 1)]);

/** `look` 外观下的统计卡片：分组底色，手机上圆角 20px、2-in-1 上 12px，没有边框和阴影。 */
export function statisticsCardClass(look: StatisticsLook): string {
  return `bg-[var(--p-group-bg)] ${look === "hm2" ? "rounded-[12px]" : "rounded-[20px]"}`;
}
