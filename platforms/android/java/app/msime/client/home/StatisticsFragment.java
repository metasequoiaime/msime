package app.msime.client.home;

import android.os.Bundle;
import android.view.LayoutInflater;
import android.view.MenuItem;
import android.view.SubMenu;
import android.view.View;
import android.view.ViewGroup;
import android.widget.PopupMenu;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import app.msime.client.R;
import app.msime.client.keyboard.KeyboardGeometry;
import app.msime.client.TypingStatisticsModel;
import app.msime.client.TypingStatisticsModel.Section;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.button.MaterialButtonToggleGroup;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.time.LocalDate;
import java.util.List;
import java.util.Map;

/**
 * The 统计 tab: today and the running total, then one of four views over the same counts.
 *
 * The counts come from the shared typing-statistics store, which holds aggregate counts only --
 * never the text that produced them. Nothing here is filled in with placeholder numbers: a store
 * that has never been written says so, because a zero and "not recording" mean different things to
 * the reader.
 */
public final class StatisticsFragment extends HomeTabFragment {
    /** 趋势默认画 30 天；记录不足 30 天就画到最早那条。 */
    /** 趋势最多画一年；这是图表的宽度上限，不是保留期限。 */
    private static final int TREND_DAY_LIMIT = 366;
    /** 下限三十天，和 Apple 一致：一个月以下看不出「这个月比上个月多」。 */
    private static final int TREND_DAY_FLOOR = 30;

    private static final Map<String, String> RETENTIONS = retentions();

    private Section section = Section.TREND;
    @Nullable private TypingStatisticsModel statistics;
    @Nullable private String selectedDay;

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_statistics, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        // The design's segmented control over the four views; the current one is checked before the listener goes on, so building it does not render twice.
        MaterialButtonToggleGroup ranges = view.findViewById(R.id.statistics_ranges);
        Section[] sections = Section.values();
        int[] segments = new int[sections.length];
        for (int index = 0; index < sections.length; index++) {
            MaterialButton segment = (MaterialButton) getLayoutInflater()
                .inflate(R.layout.item_segment, ranges, false);
            segment.setId(View.generateViewId());
            segment.setText(sections[index].tab());
            ranges.addView(segment);
            segments[index] = segment.getId();
        }
        ranges.check(segments[section.ordinal()]);
        ranges.addOnButtonCheckedListener((group, id, checked) -> {
            if (!checked) return;
            for (int index = 0; index < segments.length; index++) {
                if (segments[index] != id) continue;
                section = sections[index];
                render();
            }
        });

        view.findViewById(R.id.statistics_menu).setOnClickListener(this::showMenu);

        HeatmapView heatmap = view.findViewById(R.id.statistics_heatmap);
        heatmap.setOnDayPicked(picked -> {
            // Tapping the same day again, or a cell with nothing in it, returns to the total: the
            // selection is a lens, and there has to be a way back that is not a hunt for a button.
            selectedDay = picked == null || picked.equals(selectedDay) ? null : picked;
            render();
        });
        view.findViewById(R.id.statistics_scope_clear).setOnClickListener(ignored -> {
            selectedDay = null;
            render();
        });

        reload();
    }

    /**
     * 记录开关、保留期、刷新和清空都收在这后面。
     *
     * <p>They used to sit in a card under whichever tab was open, so every switch between 趋势 and
     * 方案 meant scrolling past the same three controls again. This page is for reading numbers;
     * the controls are for the once in a while you change something. The Apple app moved them to
     * the same place for the same reason.
     */
    private void showMenu(View anchor) {
        if (statistics == null) return;
        PopupMenu menu = new PopupMenu(requireContext(), anchor);
        MenuItem record = menu.getMenu().add("记录打字统计");
        record.setCheckable(true);
        record.setChecked(statistics.enabled());
        record.setOnMenuItemClickListener(item -> {
            boolean next = !statistics.enabled();
            HostTask.run(this, context -> HostStore.setStatisticsEnabled(context, next), this::adopt);
            return true;
        });
        SubMenu retention = menu.getMenu().addSubMenu("保留每日明细");
        for (Map.Entry<String, String> entry : RETENTIONS.entrySet()) {
            String key = entry.getKey();
            MenuItem item = retention.add(entry.getValue());
            item.setCheckable(true);
            item.setChecked(key.equals(statistics.retention()));
            item.setOnMenuItemClickListener(ignored -> {
                HostTask.run(this, context -> HostStore.setStatisticsRetention(context, key),
                    this::adopt);
                return true;
            });
        }
        retention.setGroupCheckable(0, true, true);
        menu.getMenu().add("刷新统计").setOnMenuItemClickListener(item -> {
            reload();
            return true;
        });
        menu.getMenu().add("清空统计").setOnMenuItemClickListener(item -> {
            new MaterialAlertDialogBuilder(requireContext())
                .setTitle("清除全部统计")
                .setMessage("今日、累计和全部分类计数都会归零，且无法恢复。键盘会从下一次输入重新开始记录。")
                .setNegativeButton("取消", null)
                .setPositiveButton("清除", (dialog, which) -> HostTask.run(this,
                    context -> HostStore.resetStatistics(context), this::adopt))
                .show();
            return true;
        });
        menu.show();
    }

    // The keyboard is a separate process and has been recording while this screen was away.
    @Override protected void onBecameVisible() {
        if (statistics != null) reload();
    }

    private void reload() {
        HostTask.run(this, HostStore::loadStatistics, this::adopt);
    }

    private void adopt(@Nullable TypingStatisticsModel value) {
        if (value != null) statistics = value;
        render();
    }

    private void render() {
        View view = getView();
        if (view == null) return;
        TextView today = view.findViewById(R.id.statistics_today);
        TextView total = view.findViewById(R.id.statistics_total);
        TextView notice = view.findViewById(R.id.statistics_state);
        View trendSection = view.findViewById(R.id.statistics_trend_section);
        View distributionSection = view.findViewById(R.id.statistics_distribution_section);

        if (statistics == null) {
            today.setText("—");
            total.setText("—");
            notice.setVisibility(View.VISIBLE);
            notice.setText("还没有记录。开始用键盘输入后，这里会出现每日字符数；统计只保存聚合计数，不保存输入内容。");
            trendSection.setVisibility(View.GONE);
            distributionSection.setVisibility(View.GONE);
            return;
        }

        String day = LocalDate.now().toString();
        today.setText(String.valueOf(statistics.count(day)));
        // 选中某一天时，右边那个数字跟着分类一起换成那一天；否则它是累计。
        boolean scoped = selectedDay != null;
        ((TextView) view.findViewById(R.id.statistics_scope_title))
            .setText(scoped ? readableDay(selectedDay) : "累计输入");
        total.setText(String.valueOf(scoped ? statistics.count(selectedDay) : statistics.total()));
        notice.setVisibility(statistics.enabled() ? View.GONE : View.VISIBLE);
        notice.setText("记录已关闭。已有的计数保留在本机，新的输入不再计入。");
        MaterialButton scope = view.findViewById(R.id.statistics_scope_clear);
        scope.setVisibility(scoped ? View.VISIBLE : View.GONE);

        boolean trend = section == Section.TREND;
        trendSection.setVisibility(trend ? View.VISIBLE : View.GONE);
        distributionSection.setVisibility(trend ? View.GONE : View.VISIBLE);
        if (trend) {
            // 画到最早那条记录为止，上限一年：数据本来就攒着一年，固定三十天看不出月与月之间的差。
            int span = statistics.recordedSpan(day);
            int days = span <= 0 ? TREND_DAY_FLOOR
                : KeyboardGeometry.bounded(span, TREND_DAY_FLOOR, TREND_DAY_LIMIT);
            int[] series = statistics.trend(day, days);
            TrendChart chart = view.findViewById(R.id.statistics_trend);
            ((TextView) view.findViewById(R.id.statistics_trend_title))
                .setText(days >= 360 ? "每日趋势 · 近一年" : "每日趋势 · 近 " + days + " 天");
            chart.setDaily(series);
            chart.setContentDescription("每日趋势，近 " + days + " 天，最高 " + peak(series) + " 字符");
            ((TextView) view.findViewById(R.id.statistics_trend_peak))
                .setText("最高 " + peak(series) + " 字符 / 天");
            HeatmapView heatmap = view.findViewById(R.id.statistics_heatmap);
            int calendarDays = TREND_DAY_LIMIT;
            heatmap.setDaily(statistics.trend(day, calendarDays));
            heatmap.setDays(statistics.trendDays(day, calendarDays));
            heatmap.setSelected(selectedDay);
            heatmap.setContentDescription(selectedDay == null
                ? "输入日历，每天一格，点按查看单日分类"
                : "输入日历，已选中 " + readableDay(selectedDay));
            return;
        }
        List<TypingStatisticsModel.Slice> slices = statistics.slices(section, selectedDay);
        ((TextView) view.findViewById(R.id.statistics_distribution_title))
            .setText(selectedDay == null ? section.heading()
                : section.heading() + " · " + readableDay(selectedDay));
        // 设计稿里类型和模式都是环形图，环心放总数；方案仍按排行画，因为它能有十五行，放进环里大半是细得看不见的弧。
        DistributionView.Style chart = switch (section) {
            case KIND, MODE -> DistributionView.Style.DONUT;
            default -> DistributionView.Style.RANK;
        };
        ((DistributionView) view.findViewById(R.id.statistics_distribution))
            .setSlices(slices, chart);
        ((TextView) view.findViewById(R.id.statistics_distribution_note)).setText(note(section));
    }

    private static int peak(int[] series) {
        int peak = 0;
        for (int value : series) peak = Math.max(peak, value);
        return peak;
    }

    /** `2026-09-21` as `9 月 21 日`; the year is only worth printing when it is not this one. */
    private static String readableDay(String day) {
        try {
            LocalDate date = LocalDate.parse(day);
            String text = date.getMonthValue() + " 月 " + date.getDayOfMonth() + " 日";
            return date.getYear() == LocalDate.now().getYear() ? text : date.getYear() + " 年 " + text;
        } catch (java.time.format.DateTimeParseException error) {
            return day;
        }
    }

    private static String note(Section section) {
        return switch (section) {
            case KIND -> "按上屏字符本身分类。组合表情算一个字符，历史记录里没有分类的计入「历史未分类」。";
            case MODE -> "按提交时使用的键盘模式统计，不推测文本语言；中文模式下输入的数字仍计入中文模式。AI 润色和语音输入单独按来源统计。";
            case SCHEME -> "拼音方案统计其上屏字符数，不计未上屏的拼音按键。旧版本总数保留为历史未分类，新输入开始记录细分。";
            case TREND -> "";
        };
    }

    private static Map<String, String> retentions() {
        java.util.LinkedHashMap<String, String> values = new java.util.LinkedHashMap<>();
        values.put("forever", "一直保留");
        values.put("365d", "一年");
        values.put("180d", "半年");
        values.put("90d", "90 天");
        values.put("30d", "30 天");
        return java.util.Collections.unmodifiableMap(values);
    }
}
