import { StatisticsAuthenticationGate } from "./statistics-authentication-gate";
import {
  retentionChoices,
  activityMetrics,
  usualHours,
  dailySpeeds,
  dailyDetailRows,
  type StatisticsRetention,
  type TypingStatistics,
  type TypingStatisticsStatus,
  type TypingStatisticsClient,
} from "./typing-statistics-model";
export {
  HOURS,
  DETAIL_DAYS,
  retentionChoices,
  activityMetrics,
  usualHours,
  dailySpeeds,
  dailyDetailRows,
  type SelectionCounts,
  type StatisticsRetention,
  type TypingStatistics,
  type TypingStatisticsStatus,
  type TypingStatisticsClient,
  type ActivityMetrics,
  type DailyDetailRow,
} from "./typing-statistics-model";
import { rowHeading } from "../core/row-style";
import { compactPage } from "../core/page-style";
import { useMemo, useState } from "react";
import { useConfirm } from "../core/confirm";
import { KeyboardHeatmap, StatisticsHeatmap } from "./statistics-heatmaps";
import { SpeedTrend, StatisticsDailyTrend, StatisticsHourlyBars } from "./statistics-trends";
import { scopedBreakdown } from "./typing-breakdown";
import { statisticsDistributionSlices } from "./statistics-distribution-model";
import { CandidateRanks, Distribution } from "./statistics-distributions";
import { SelectSettingField } from "./select-setting-field";
import { SettingToggle } from "./setting-toggle";
import { ActionButton } from "./action-button";
import { ErrorAlert } from "../core/error-alert";
import { StatusMessage } from "../core/status-message";
import { formatDateTime } from "../core/format-date";
import { useTypingStatisticsState } from "./use-typing-statistics-state";
import { MobileStatistics, statisticsCardClass, type StatisticsLook } from "./mobile-statistics";
import { StatisticsNote } from "./statistics-note";
import { StatisticsOptionsMenu } from "./statistics-options-menu";
import { StatisticsDailyDetails } from "./statistics-daily-details";
import { StatisticsSection } from "./statistics-section";
import { TypingStatisticsUpload } from "./typing-statistics-upload";
export type { TypingBreakdown } from "./typing-speed";
export type { TypingSummary } from "./typing-summary";
import type { TouchKeyLayoutStyle } from "../keyboard/touch-key-layout-styles";
import { dayLabel, mobileTrendLength, recentDays } from "./typing-statistics-helpers";
import {
  StatisticsMetric,
  statisticsOverviewDetails,
  statisticsOverviewMetrics,
} from "./typing-statistics-overview";
export {
  addDays,
  currentStreak,
  formatActiveTime,
  longestStreak,
  statisticDayKeys,
} from "./typing-statistics-helpers";
export {
  statisticsOverviewDetails,
  statisticsOverviewMetrics,
  type StatisticsOverviewMetric,
} from "./typing-statistics-overview";

const heading = rowHeading;
// 摘要在标签行上方，切换标签时保持不动，所以压成紧凑的 3 × 2 网格，单位和数字放在同一行；会随标签变化的内容都放在标签下方。
const metricGrid = "grid grid-cols-3 gap-x-6 gap-y-[18px] max-phone:grid-cols-2 max-phone:gap-x-3";
const privacy = "mt-4 mb-0 text-xs leading-[1.7] [color:var(--p-sub)]";
// The page container. Named because this component returns it from three places -- loading, error and
// loaded -- and the loading branch was the one that got left behind when the class it used was
// replaced.
const page = compactPage;
// HarmonyOS 外观下的纵向排列：分段控件、各卡片组和隐私说明之间按设计间隔 22px。
const summaryPage = "flex flex-col gap-[22px] text-[color:var(--p-text)]";

// The segmented control behind the content tabs. The column count is a parameter because the tab row silently kept four columns after a fifth tab was added -- the extra one wrapped onto a second row at a quarter width. Each count is spelled out so Tailwind sees the class.
const segmentedColumns: Record<number, string> = {
  5: "grid-cols-5",
  6: "grid-cols-6",
};
const segmented = (columns: number) =>
  `grid gap-[3px] rounded-[9px] bg-subtle p-[3px] ${segmentedColumns[columns]} [&>button]:min-h-[34px] [&>button]:rounded-[7px] [&>button]:border-0 [&>button]:bg-transparent [&>button]:text-secondary [&>button[aria-selected=true]]:bg-raised [&>button[aria-selected=true]]:text-body [&>button[aria-selected=true]]:shadow-card`;

/** Days the desktop trend bars cover. */
const DESKTOP_TREND_DAYS = 30;
/** Why the page has nothing to show, or "" when there is nothing to explain.
 *
 * Every message here ends in "type a few more characters and come back". That is only true advice
 * while recording is on: with it off the counts are zero because nothing is being recorded, and
 * typing more records nothing. Recording ships off, so that is the state a new profile lands in -
 * it gets the call to action above instead, and this stays quiet rather than sending anyone off to
 * type for no effect.
 *
 * On iOS the keyboard extension cannot reach the shared App Group container without Full Access,
 * so there the prerequisite is named rather than the typing.
 *
 * iOS 上统计文件存在也不能说明键盘写过：在 app 里开启记录时 `client-services` 的 `set_enabled` 会无条件写出统计文件，而没有完全访问的键盘什么都不记，计数一直是零。app 读不到键盘是否有完全访问，所以计数为零时也先提醒这一项，再说其他可能。
 */
export function availabilityNotice(
  statistics: Pick<TypingStatistics, "enabled" | "total">,
  status: Pick<TypingStatisticsStatus, "availability" | "lastWrittenMs">,
  iosPlatform: boolean,
): string {
  if (!statistics.enabled) return "";
  if (status.availability === "neverWritten")
    return iosPlatform
      ? "键盘从未写入过统计。请在系统设置 → 通用 → 键盘 → 键盘 → 水杉输入法中开启“允许完全访问”，然后用水杉键盘输入几个字再回来刷新。未开启时仍可正常打字，只是不记录统计。"
      : "键盘从未写入过统计。请用水杉键盘成功输入几个字符，再返回此页刷新。";
  if (statistics.total !== 0) return "";
  const fullAccess = iosPlatform
    ? "请先确认已在系统设置 → 通用 → 键盘 → 键盘 → 水杉输入法中为水杉键盘开启“允许完全访问”，未开启时键盘不记录统计。"
    : "";
  if (status.lastWrittenMs) {
    const written = formatDateTime(status.lastWrittenMs, "zh-CN");
    return iosPlatform
      ? `统计最后写入于 ${written}，当前计数为零。${fullAccess}已开启的话，如果刚刚清空过统计，这是正常的。`
      : `统计最后写入于 ${written}，当前计数为零；如果刚刚清空过统计，这是正常的。`;
  }
  return `统计文件已建立，但当前还没有输入记录。${fullAccess}`;
}

export function TypingStatisticsPage(props: TypingStatisticsPageProps) {
  if (props.client.authentication) {
    return (
      <StatisticsAuthenticationGate client={props.client.authentication}>
        <TypingStatisticsContent {...props} />
      </StatisticsAuthenticationGate>
    );
  }
  return <TypingStatisticsContent {...props} />;
}

type TypingStatisticsPageProps = {
  client: TypingStatisticsClient;
  mobile?: boolean;
  platform?: string;
  /** iOS 共享面板打开系统设置；原生 iOS App 不使用此页面。 */
  openSystemSettings?: () => Promise<void>;
  /** 鸿蒙统计摘要使用「概览 / 习惯 / 按键 / 成就」四个标签。 */
  look?: StatisticsLook;
  /** HarmonyOS 手机上用户当前的按键布局样式与数字行，按键热力图照它画；不传时画默认键盘。 */
  layoutStyle?: TouchKeyLayoutStyle;
  /** 宿主提供 AI 生成类功能（`HostCapabilities.ai_generation`）。为 false 时（鸿蒙版拿到资质之前）分布里不列 AI 润色和高情商回复，说明文字也不提它们；缺省为提供。 */
  aiGeneration?: boolean;
};

function TypingStatisticsContent({
  client,
  mobile = false,
  platform,
  openSystemSettings,
  look,
  layoutStyle,
  aiGeneration = true,
}: TypingStatisticsPageProps) {
  const { confirm, confirmation } = useConfirm();
  const { status, today, busy, error, summary, summaryFailed, update, openDataDirectory } =
    useTypingStatisticsState(client, look);
  const [selectedDay, setSelectedDay] = useState<string | null>(null);
  // 打开页面时先看按键热力图，它排在标签行的第一位；标签只存在组件状态里，不跨次打开记忆。
  const [contentTab, setContentTab] = useState<
    "keys" | "trend" | "kind" | "mode" | "scheme" | "ranks"
  >("keys");
  // 「上传到云端」打开时，页面上「只保存在本机」的说明要改写。
  const [uploadEnabled, setUploadEnabled] = useState(false);
  const summaryLook = look && client.summary ? look : undefined;
  const mobileTrendDays = useMemo(
    () => recentDays(mobileTrendLength(status?.statistics.days ?? {})),
    [status?.statistics.days, today.key],
  );
  const desktopTrendDays = useMemo(() => recentDays(DESKTOP_TREND_DAYS), [today.key]);
  const trendDays = mobile ? mobileTrendDays : desktopTrendDays;

  if (!status)
    return (
      <div className={page}>
        {error ? (
          <ErrorAlert>{error}</ErrorAlert>
        ) : (
          <StatusMessage role="status">正在读取打字统计…</StatusMessage>
        )}
      </div>
    );
  const statistics = status.statistics;
  const scopeKeys = selectedDay ? [selectedDay] : null;
  const breakdown = scopedBreakdown(statistics, scopeKeys);
  const scopeTotal =
    scopeKeys === null
      ? statistics.total
      : scopeKeys.reduce((total, key) => total + (statistics.days[key] ?? 0), 0);
  const selectedLabel = selectedDay
    ? (trendDays.find((day) => day.key === selectedDay)?.label ?? dayLabel(selectedDay))
    : null;
  const scopeTitle = selectedLabel ?? "累计输入";
  const keyScopeLabel = selectedLabel ?? "累计";
  const activity = activityMetrics(statistics, today.key);
  const { characterSlices, sourceSlices, languageSlices } = statisticsDistributionSlices(
    breakdown,
    {
      aiGeneration,
    },
  );
  const iosPlatform = platform === "ios";
  const availabilityMessage = availabilityNotice(statistics, status, iosPlatform);
  const resetStatistics = async () => {
    const confirmed = await confirm({
      title: "清空打字统计",
      message: summaryLook
        ? "累计字数、分类、每日记录、按键次数和已解锁的成就都会被删除，无法恢复。"
        : "累计字数、分类、每日记录和按键次数都会被删除，无法恢复。",
      confirmLabel: "清空",
      danger: true,
    });
    if (!confirmed) return;
    setSelectedDay(null);
    await update(client.reset);
  };
  const menuActions = {
    enabled: statistics.enabled,
    busy,
    onEnabledChange: (enabled: boolean) => void update(() => client.setEnabled(enabled)),
    onRefresh: () => void update(() => client.load(), true),
    onReset: () => void resetStatistics(),
  };

  if (summaryLook) {
    const card = statisticsCardClass(summaryLook);
    return (
      <div className={summaryPage}>
        {confirmation}
        {error && <ErrorAlert>{error}</ErrorAlert>}
        {/* 摘要外观的图表下方没有设置区，保留期选项只在这里进入菜单。 */}
        <StatisticsOptionsMenu
          {...menuActions}
          className="-mb-3.5"
          retention={statistics.retention}
          onRetentionChange={
            client.setRetention
              ? (chosen) => {
                  const setRetention = client.setRetention;
                  if (setRetention) void update(() => setRetention(chosen));
                }
              : undefined
          }
        />
        {!statistics.enabled && (
          <section
            className={`${card} flex flex-col items-start gap-2 p-4`}
            aria-labelledby="statistics-disabled-title"
          >
            <h2 id="statistics-disabled-title" className="m-0 text-[15px] font-semibold">
              输入统计已关闭
            </h2>
            <p className="m-0 text-[13px] leading-relaxed text-[color:var(--p-sub)]">
              已有的计数保留在本机，新的输入不再计入。开启后这里会显示输入字数、速度、时段分布、按键热力图与成就。
            </p>
            <ActionButton
              action={() => void update(() => client.setEnabled(true))}
              ariaBusy={busy}
              className="secondary m-0"
              disabled={busy}
              label={busy ? "处理中…" : "启用输入统计"}
            />
          </section>
        )}
        {availabilityMessage && (
          <section className={`${card} flex flex-col items-start gap-2 p-4`}>
            <h2 className="m-0 text-[15px] font-semibold">统计没有数据</h2>
            <p className="m-0 text-[13px] leading-relaxed text-[color:var(--p-sub)]">
              {availabilityMessage}
            </p>
          </section>
        )}
        {summary ? (
          <MobileStatistics
            look={summaryLook}
            summary={summary}
            dailyKeys={statistics.dailyKeys}
            uploading={uploadEnabled}
            layoutStyle={layoutStyle}
          />
        ) : summaryFailed ? (
          <ErrorAlert>统计暂时读不到，可以在右上角菜单里刷新。</ErrorAlert>
        ) : (
          <StatusMessage role="status">正在读取打字统计…</StatusMessage>
        )}
        {/* 鸿蒙外观的图表下面没有设置区，「上传到云端」单独占一张卡片，排在最后。 */}
        {client.cloudUpload && (
          <section className={`${card} px-4 pt-4`}>
            <TypingStatisticsUpload
              client={client.cloudUpload}
              onEnabledChange={setUploadEnabled}
            />
          </section>
        )}
      </div>
    );
  }

  return (
    <div className={page}>
      {confirmation}
      {error && <ErrorAlert>{error}</ErrorAlert>}
      {mobile && <StatisticsOptionsMenu {...menuActions} className="-mb-1" />}
      {!statistics.enabled && (
        <StatisticsSection id="statistics-disabled-title" title="输入统计已关闭">
          <p className="mt-2 mb-0 leading-relaxed text-secondary">
            {uploadEnabled
              ? "开启后这里会显示输入字数、速度、时段分布与按键热力图。统计不记录输入内容；已打开「上传到云端」，每日计数会上传到你的水杉账号。"
              : "开启后这里会显示输入字数、速度、时段分布与按键热力图。统计只保存在本机，不记录输入内容，也不联网。"}
          </p>
          <ActionButton
            action={() => void update(() => client.setEnabled(true))}
            ariaBusy={busy}
            className="secondary"
            disabled={busy}
            label={busy ? "处理中…" : "启用输入统计"}
          />
        </StatisticsSection>
      )}
      <section className="section m-0" aria-label="统计概览">
        <div className={metricGrid}>
          {statisticsOverviewMetrics({
            statistics,
            todayKey: today.key,
            scopeTitle,
            scopeTotal,
            activity,
          }).map((metric) => (
            <StatisticsMetric key={metric.ariaLabel} metric={metric} />
          ))}
        </div>
        {/* Windows 的概览在六格下面并排画「日均…」和「最多…·最快…」两行；共享页把日均放进了「累计输入」的说明，最好成绩这行一直没画。这里按 Windows 的措辞补上，靠右对齐。 */}
        <p className="mt-3.5 mb-0 text-right text-[11px] leading-relaxed [color:var(--p-sub)]">
          {statisticsOverviewDetails(activity).best}
        </p>
        <p className="mt-4 mb-0 border-t border-edge pt-3 text-[11px] leading-relaxed [color:var(--p-sub)]">
          {activity.hasActivity
            ? "速度统计汉字与字母；两次上屏间隔超过 10 秒视为休息，不计入活跃时长。"
            : "还没有测量到活跃时长。这项从本次更新后开始记录，之前的输入只有字数。"}
        </p>
      </section>
      <div className={segmented(6)} role="tablist" aria-label="统计内容">
        {(
          [
            ["keys", "按键"],
            ["trend", "趋势"],
            ["kind", "类型"],
            ["mode", "模式"],
            ["scheme", "方案"],
            ["ranks", "候选"],
          ] as const
        ).map(([value, label]) => (
          <button
            type="button"
            role="tab"
            key={value}
            aria-selected={contentTab === value}
            onClick={() => {
              setContentTab(value);
              // 桌面端切换标签时保留选中的那一天：它的柱子在「趋势」里，而其他标签正是用来看这一天的分项。
              if (mobile) setSelectedDay(null);
            }}
          >
            {label}
          </button>
        ))}
      </div>
      {contentTab === "trend" && (
        <StatisticsSection
          id="statistics-trend-title"
          title={`每日趋势 · ${mobile && trendDays.length >= 360 ? "近一年" : `近 ${trendDays.length} 天`}`}
          help={mobile ? "点按热力图查看当天的分类与占比。" : "点按柱形查看当天的分类与占比。"}
        >
          <StatisticsDailyTrend
            days={trendDays}
            counts={statistics.days}
            selectedDay={selectedDay}
            onSelect={(key) => setSelectedDay((current) => (current === key ? null : key))}
            mobile={mobile}
          />
          {mobile && (
            <>
              <p className="mt-[18px] mb-0 text-xs text-muted">每天一格，一列一周</p>
              <StatisticsHeatmap
                days={statistics.days}
                selectedDay={selectedDay}
                onSelect={(key) => setSelectedDay((current) => (current === key ? null : key))}
              />
            </>
          )}
          {selectedDay && (
            <ActionButton action={() => setSelectedDay(null)} label="返回整个时间范围" />
          )}
        </StatisticsSection>
      )}
      {contentTab === "trend" && activity.todayHours && (
        <StatisticsSection id="statistics-hours-title" title="今日时段">
          <StatisticsHourlyBars
            hours={activity.todayHours}
            usual={usualHours(statistics.dailyHours, today.key)}
          />
        </StatisticsSection>
      )}
      {contentTab === "trend" && (
        <StatisticsSection id="statistics-speed-title" title="速度趋势">
          <SpeedTrend
            days={trendDays}
            speeds={dailySpeeds(
              statistics,
              trendDays.map((day) => day.key),
            )}
            averageSpeed={activity.averageSpeed}
            selectedDay={selectedDay}
          />
        </StatisticsSection>
      )}
      {!mobile && contentTab === "trend" && (
        <StatisticsSection id="statistics-calendar-title" title="日历热力图">
          <StatisticsNote>近 12 个月，颜色越深输入越多</StatisticsNote>
          <StatisticsHeatmap
            days={statistics.days}
            selectedDay={selectedDay}
            onSelect={(key) => setSelectedDay((current) => (current === key ? null : key))}
          />
        </StatisticsSection>
      )}
      {contentTab === "keys" && (
        <KeyboardHeatmap
          dailyKeys={statistics.dailyKeys}
          scopeKeys={scopeKeys}
          scopeLabel={keyScopeLabel}
          mobile={mobile}
          platform={platform}
        />
      )}
      {contentTab === "kind" && (
        <Distribution title="字符类型" slices={characterSlices} variant="donut" wide={!mobile} />
      )}
      {contentTab === "mode" && (
        <Distribution
          title="语言模式"
          slices={languageSlices}
          variant="donut"
          wide={!mobile}
          footer={
            aiGeneration
              ? "按提交时使用的键盘模式统计，不推测文本语言；中文模式下输入的数字仍计入中文模式。AI 润色和语音输入单独按来源统计。"
              : "按提交时使用的键盘模式统计，不推测文本语言；中文模式下输入的数字仍计入中文模式。语音输入单独按来源统计。"
          }
        />
      )}
      {contentTab === "scheme" && (
        <Distribution
          title="输入方案"
          slices={sourceSlices}
          variant="rank"
          footer="输入方案统计其上屏字符数；拼音等按键另由按键热力图计数，只记每个键每天的按下次数。来源无法归类的字数计入历史未分类。"
        />
      )}
      {!mobile && contentTab === "trend" && (
        <StatisticsDailyDetails rows={dailyDetailRows(statistics, today.key)} />
      )}
      {contentTab === "ranks" && <CandidateRanks selections={statistics.selections} />}
      {mobile ? (
        <section className="section m-0 pt-0.5">
          {client.cloudUpload && (
            <TypingStatisticsUpload
              client={client.cloudUpload}
              onEnabledChange={setUploadEnabled}
            />
          )}
          <p className={`${privacy} mt-0`}>
            字数仅统计水杉键盘成功提交的字符，含标点及表情，不含空格和换行。组合表情计为一个字符，删除文字不扣减。按键热力图只保存每个键每天被按下的次数，不保存按键顺序和输入内容。
            {uploadEnabled
              ? "日期、分类和数量保存在本机，不保存输入内容；已打开「上传到云端」，最近 7 天的每日计数和候选位次会上传到你的水杉账号，按键热力图不上传。"
              : "仅在本机保存日期、分类和数量，不保存输入内容。"}
            每日明细默认永久保留。
          </p>
        </section>
      ) : (
        <section className="section m-0">
          <SettingToggle
            label="记录打字统计"
            // 统计记什么、存在哪里、保留多久：原先是页末一段常驻灰字，现在和开关的说明一起收进「?」。
            description={
              <>
                <span className="block">关闭后，新提交不会增加统计。</span>
                <span className="mt-1.5 block">
                  字数统计水杉键盘提交的字符，以及英文模式和放行给应用的字母、数字与符号（按按键时估计），含标点及表情，不含空格和换行。组合表情计为一个字符，删除文字不扣减。按键热力图只保存每个键每天被按下的次数，不保存按键顺序和输入内容。
                  {uploadEnabled
                    ? "日期、分类和数量保存在本机，不保存输入内容；已打开「上传到云端」，最近 7 天的每日计数和候选位次会上传到你的水杉账号，按键热力图不上传。"
                    : "仅在本机保存日期、分类和数量，不保存输入内容。"}
                  每日明细默认永久保留，可在「自动清理」中改为只保留最近一段时间；清理删除的日期同时从累计总数与分类中扣除。
                </span>
              </>
            }
            ariaLabel="记录打字统计"
            rowClassName="mb-4"
            compact
            checked={statistics.enabled}
            disabled={busy}
            onChange={(enabled) => void update(() => client.setEnabled(enabled))}
          />
          {client.cloudUpload && (
            <TypingStatisticsUpload
              client={client.cloudUpload}
              onEnabledChange={setUploadEnabled}
            />
          )}
          {client.setRetention && (
            <SelectSettingField
              label="自动清理"
              inputLabel="自动清理"
              description="按保留策略删除超期的每日记录并从累计中扣除，跨天后首次记录时执行。"
              fieldClassName="section-header mb-4"
              value={statistics.retention ?? "forever"}
              disabled={busy}
              onChange={(value) => {
                const setRetention = client.setRetention;
                if (!setRetention) return;
                const chosen = value as StatisticsRetention;
                void update(() => setRetention(chosen));
              }}
            >
              {retentionChoices.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </SelectSettingField>
          )}
          {/* 桌面没有「刷新统计」：页面每 5 秒自动重读一次，窗口回到前台时也会重读。手机的溢出菜单仍保留它。 */}
          <div className="flex flex-wrap gap-[9px]">
            {client.openDirectory && (
              <ActionButton
                action={openDataDirectory}
                ariaBusy={busy}
                className="secondary m-0"
                disabled={busy}
                label="打开数据目录"
              />
            )}
            <ActionButton
              action={() => void resetStatistics()}
              ariaBusy={busy}
              className="secondary m-0 text-danger"
              disabled={busy}
              label="清空统计"
            />
          </div>
        </section>
      )}
      {availabilityMessage && (
        <section className="section m-0">
          <h2 className={heading}>统计没有数据</h2>
          <p className="mt-2 mb-0 leading-relaxed text-secondary">{availabilityMessage}</p>
          {iosPlatform && openSystemSettings && (
            <ActionButton action={openSystemSettings} label="打开系统键盘设置" />
          )}
        </section>
      )}
    </div>
  );
}
