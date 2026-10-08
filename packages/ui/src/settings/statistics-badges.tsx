import { useState, type CSSProperties } from "react";
import { useToast } from "../core/toast";
import {
  badgeToast,
  caption,
  progressLabel,
  unlocked,
  type AchievementGroup,
  type AchievementSummary,
} from "./typing-summary";

/** 强调色与另一种颜色的 `color-mix`，即设计稿的 `mx2(percent, colour)`。 */
const mixAccent = (percent: number, colour: string) =>
  `color-mix(in srgb, var(--accent-color) ${percent}%, ${colour})`;

/**
 * 每组奖章的颜色，深色一端在前：数量组用强调色本身，连续组用加深的强调色，技巧组混入蓝色，趣味组混入紫色，与原型和 Android 的 `BadgeGridView` 配色一致。颜色由强调色生成，所以奖章跟随季节主题。
 */
const groupColours: Record<AchievementGroup, [string, string]> = {
  volume: ["var(--accent-color)", mixAccent(70, "#FFFFFF")],
  streak: [mixAccent(82, "#000000"), mixAccent(80, "#FFFFFF")],
  skill: [mixAccent(70, "#3A6EA5"), mixAccent(55, "#FFFFFF")],
  fun: [mixAccent(68, "#7A5BA8"), mixAccent(55, "#FFFFFF")],
};

/** 奖章文字随长度缩小，让「百万」和「10万」也能放进菱形。 */
function glyphSize(glyph: string): string {
  const length = [...glyph].length;
  return length > 2 ? "13px" : length > 1 ? "15px" : "19px";
}

/** 进度条或进度环的空白部分，即设计稿的 `stTrack`。 */
const trackClass = "bg-[rgba(255,255,255,0.1)] light-theme:bg-[rgba(0,0,0,0.07)]";

/** 奖章的动效：网格首次绘制时弹入，点按时晃动，之后保持静止，这样晃动结束后不会重放弹入。 */
type MedalMotion = "pop" | "wiggle" | "rest";

/** 奖章的倾斜是每个关键帧 `transform` 的一部分，所以静止时的倾斜也写成 `transform`；`rotate-45` 工具类设置的是独立的 `rotate` 属性，会再叠加一个 45°。 */
const medalMotion: Record<MedalMotion, string> = {
  pop: "animate-ms-medal-pop",
  wiggle: "animate-ms-wiggle",
  rest: "",
};

function Medal({
  badge,
  index,
  motion,
  onWiggleEnd,
}: {
  badge: AchievementSummary;
  index: number;
  motion: MedalMotion;
  onWiggleEnd: () => void;
}) {
  const [deep, light] = groupColours[badge.group] ?? groupColours.volume;
  const delay = `${index * 45}ms`;
  return (
    <span
      className={`relative flex size-[54px] shrink-0 items-center justify-center overflow-hidden rounded-[17px] [transform:rotate(45deg)] motion-reduce:animate-none ${medalMotion[motion]}`}
      style={{
        background: `linear-gradient(145deg, ${light} 0%, ${deep} 70%)`,
        boxShadow: `0 4px 12px color-mix(in srgb, ${deep} 33%, transparent), inset 0 1px 0 rgba(255,255,255,0.35)`,
        animationDelay: motion === "pop" ? `${index * 45 + 120}ms` : undefined,
      }}
      onAnimationEnd={(event) => {
        if (motion === "wiggle" && event.target === event.currentTarget) onWiggleEnd();
      }}
      aria-hidden="true"
    >
      <span
        className="absolute top-[-20%] left-0 h-[140%] w-[34%] animate-ms-shine bg-[linear-gradient(90deg,rgba(255,255,255,0),rgba(255,255,255,0.55),rgba(255,255,255,0))] motion-reduce:hidden"
        style={{ animationDelay: delay }}
      />
      <span
        className="relative -rotate-45 font-extrabold tracking-[-0.02em] text-white [text-shadow:0_1px_1px_rgba(0,0,0,0.15)]"
        style={{ fontSize: glyphSize(badge.glyph) }}
      >
        {badge.glyph}
      </span>
    </span>
  );
}

function ProgressRing({ badge, index }: { badge: AchievementSummary; index: number }) {
  const [deep] = groupColours[badge.group] ?? groupColours.volume;
  const label = progressLabel(badge);
  const style = {
    "--bp": label,
    background: `conic-gradient(${deep} var(--bp), var(--ring-track) 0)`,
    animationDelay: `${index * 45}ms`,
  } as CSSProperties;
  return (
    <span
      className="flex size-[54px] shrink-0 animate-ms-ring-fill items-center justify-center rounded-full [--ring-track:rgba(255,255,255,0.1)] motion-reduce:animate-none light-theme:[--ring-track:rgba(0,0,0,0.07)]"
      style={style}
      aria-hidden="true"
    >
      <span className="flex size-[46px] flex-col items-center justify-center gap-px rounded-full bg-[var(--p-group-bg)]">
        <span
          className="leading-none font-bold text-[color:var(--p-sub)] opacity-70"
          style={{ fontSize: glyphSize(badge.glyph) }}
        >
          {badge.glyph}
        </span>
        <span className="text-[9px] leading-none font-semibold" style={{ color: deep }}>
          {label}
        </span>
      </span>
    </span>
  );
}

/**
 * 「成就」子页：已解锁数量和进度条，下面是 3 列网格，排列核心的 16 枚徽章。已解锁的徽章是带字形的倾斜渐变菱形，未解锁的是锥形进度环；点按任一枚都会让奖章晃动，并以 toast 说明它的获得条件。移植自 Android 的 `BadgeGridView`，动效取自原型（`msBadgeIn`、`msMedalPop`、`msShine`、`msRingFill`、`msWiggle`）。
 */
export function StatisticsBadges({
  achievements,
  card,
}: {
  achievements: readonly AchievementSummary[];
  /** 当前外观的卡片表面和圆角。 */
  card: string;
}) {
  const toast = useToast();
  // 每枚徽章的点按次数：奖章以该计数为 key，所以再次点按会重新挂载奖章，重新开始晃动。
  const [taps, setTaps] = useState<Record<string, number>>({});
  const [wiggling, setWiggling] = useState<string | null>(null);
  const done = achievements.filter(unlocked).length;
  const total = achievements.length;
  return (
    <>
      <section
        className={`${card} flex flex-col gap-2 p-4`}
        aria-label={`${done} / ${total} 枚成就已解锁`}
      >
        <div className="flex items-baseline gap-1" aria-hidden="true">
          <span className="text-[28px] font-bold tabular-nums">{done}</span>
          <span className="text-[15px] text-[color:var(--p-sub)]">/ {total} 已解锁</span>
        </div>
        <div className={`h-1.5 overflow-hidden rounded-[3px] ${trackClass}`}>
          <div
            className="h-full animate-ms-bar-fill rounded-[3px] bg-[var(--accent-color)] [animation-delay:0.15s] motion-reduce:animate-none"
            style={{ width: `${total === 0 ? 0 : (done / total) * 100}%` }}
          />
        </div>
      </section>
      <div className="grid grid-cols-3 gap-2.5">
        {achievements.map((badge, index) => {
          const on = unlocked(badge);
          const tapped = taps[badge.id] ?? 0;
          return (
            <button
              type="button"
              key={badge.id}
              className={`${card} flex animate-ms-badge-in cursor-pointer flex-col items-center gap-2 border-0 px-1.5 pt-4 pb-3 text-center transition-transform duration-[120ms] active:scale-[0.96] motion-reduce:animate-none`}
              style={{ animationDelay: `${index * 45}ms` }}
              aria-label={`${badge.title}，${on ? "已解锁" : "未解锁"}，${caption(badge)}`}
              onClick={() => {
                setTaps((current) => ({ ...current, [badge.id]: (current[badge.id] ?? 0) + 1 }));
                setWiggling(badge.id);
                toast(badgeToast(badge));
              }}
            >
              {on ? (
                <Medal
                  key={tapped}
                  badge={badge}
                  index={index}
                  motion={wiggling === badge.id ? "wiggle" : tapped > 0 ? "rest" : "pop"}
                  onWiggleEnd={() =>
                    setWiggling((current) => (current === badge.id ? null : current))
                  }
                />
              ) : (
                <ProgressRing badge={badge} index={index} />
              )}
              <span
                className={`mt-0.5 text-[13px] font-semibold ${on ? "text-[color:var(--p-text)]" : "text-[color:var(--p-sub)]"}`}
                aria-hidden="true"
              >
                {badge.title}
              </span>
              <span
                className="text-[11px] leading-[1.3] text-[color:var(--p-sub)]"
                aria-hidden="true"
              >
                {caption(badge)}
              </span>
            </button>
          );
        })}
      </div>
    </>
  );
}
