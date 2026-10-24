import { useId, useMemo, useState } from "react";
import { FluentIcon } from "../core/fluent-icons";
import { scopedKeyCounts, type DailyKeyCounts } from "./keyboard-heatmap";
import { MobileStatisticsOverview } from "./mobile-statistics-overview";
import { MobileStatisticsHabits } from "./mobile-statistics-habits";
import { MobileStatisticsKeys } from "./mobile-statistics-keys";
import {
  segmentTrack,
  segmentOn,
  statisticsCardClass,
  subText,
  type StatisticsLook,
} from "./mobile-statistics-style";
import { StatisticsBadges } from "./statistics-badges";
import type { TypingSummary } from "./typing-summary";

export { statisticsCardClass, type StatisticsLook } from "./mobile-statistics-style";

type StatisticsTab = "overview" | "habits" | "keys" | "achievements";

const tabs: [StatisticsTab, string][] = [
  ["overview", "概览"],
  ["habits", "习惯"],
  ["keys", "按键"],
  ["achievements", "成就"],
];

/** 每个子标签页底部的那一行隐私说明。打开了「上传到云端」时不能再说只保存在本机。 */
function StatisticsFooter({ uploading }: { uploading: boolean }) {
  return (
    <p className={`m-0 flex items-center justify-center gap-1.5 text-[12px] ${subText}`}>
      <FluentIcon name="shield_lock" size={13} />
      {uploading
        ? "统计不包含输入内容，每日计数会上传到你的水杉账号"
        : "统计只保存在本机，不包含输入内容"}
    </p>
  );
}

/**
 * HarmonyOS 的 统计 标签页：分段控件 概览 / 习惯 / 按键 / 成就，下面是由共享的 `summary` 绘制的卡片（数字都在 Rust 里算好，这里只负责排版），最后是一行隐私说明。对应 Android 的 `StatisticsFragment`；和它一样没有 常用词 和 最常打错 卡片，因为统计从不保存输入过的内容。
 *
 * `dailyKeys` 和 Android 一样取自统计文档而不是摘要：按键热力图覆盖摘要的最近七天。
 */
export function MobileStatistics({
  look,
  summary,
  dailyKeys,
  uploading = false,
}: {
  look: StatisticsLook;
  summary: TypingSummary;
  dailyKeys: DailyKeyCounts | undefined;
  /** 打开了「上传到云端」：底部的隐私说明随之改写。 */
  uploading?: boolean;
}) {
  const [tab, setTab] = useState<StatisticsTab>("overview");
  const id = useId();
  const card = statisticsCardClass(look);
  // 把时间窗口拼成一个字符串，这样只有窗口移动或计数变化时才重新求和。
  const weekKey = summary.overview.last7.map((day) => day.day).join(",");
  const keyCounts = useMemo(
    () => scopedKeyCounts(dailyKeys, weekKey ? weekKey.split(",") : []),
    [dailyKeys, weekKey],
  );
  return (
    <>
      <div
        className={`flex rounded-[9px] p-0.5 ${segmentTrack}`}
        role="tablist"
        aria-label="统计内容"
      >
        {tabs.map(([value, label]) => {
          const on = tab === value;
          return (
            <button
              type="button"
              role="tab"
              id={`${id}-${value}-tab`}
              aria-controls={`${id}-${value}`}
              aria-selected={on}
              className={`flex h-8 flex-1 items-center justify-center rounded-[7px] border-0 text-[14px] ${on ? `${segmentOn} shadow-[0_1px_3px_rgba(0,0,0,0.12)]` : "bg-transparent text-[color:var(--p-text)]"}`}
              onClick={() => setTab(value)}
              key={value}
            >
              {label}
            </button>
          );
        })}
      </div>
      <div
        className="flex flex-col gap-[22px]"
        role="tabpanel"
        id={`${id}-${tab}`}
        aria-labelledby={`${id}-${tab}-tab`}
      >
        {tab === "overview" && <MobileStatisticsOverview overview={summary.overview} card={card} />}
        {tab === "habits" && <MobileStatisticsHabits habits={summary.habits} card={card} />}
        {tab === "keys" && (
          <MobileStatisticsKeys
            keys={summary.keys}
            habits={summary.habits}
            counts={keyCounts}
            card={card}
          />
        )}
        {tab === "achievements" && (
          <StatisticsBadges achievements={summary.achievements} card={card} />
        )}
      </div>
      <StatisticsFooter uploading={uploading} />
    </>
  );
}
