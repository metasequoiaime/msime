package app.msime.android.home;

import android.content.Context;
import android.graphics.Typeface;
import android.graphics.drawable.Drawable;
import android.os.Bundle;
import android.text.SpannableStringBuilder;
import android.text.Spanned;
import android.text.style.AbsoluteSizeSpan;
import android.text.style.ForegroundColorSpan;
import android.view.Gravity;
import android.view.LayoutInflater;
import android.view.MenuItem;
import android.view.SubMenu;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import app.msime.android.NativeClient;
import app.msime.android.BoundsPolicy;
import app.msime.android.KeyPressIds;
import app.msime.android.R;
import app.msime.android.TypingStatisticsModel;
import app.msime.android.TypingStatisticsSummary;
import app.msime.android.TypingStatisticsSummary.Achievement;
import app.msime.android.TypingStatisticsSummary.Habits;
import app.msime.android.TypingStatisticsSummary.Keys;
import app.msime.android.TypingStatisticsSummary.Overview;
import app.msime.android.TypingStatisticsSummary.Share;
import app.msime.android.policy.HostOptionsPolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.io.File;
import java.time.LocalDate;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 统计 tab：分段 概览 / 习惯 / 按键 / 成就，数字全部来自共享统计库的 `summary` 操作（口径在 Rust），页面只排版。
 *
 * <p>统计只保存聚合计数，不保存输入内容，所以设计里的「常用词」「最常打错」两张卡不做：它们需要记下用户打过的词。记录开关、保留期、刷新和清空收在右上角的菜单里。没有读到数据时页面明说，不拿 0 或示例数字填。
 */
public final class StatisticsFragment extends HomeTabFragment {
    /** 页面上的分段；和 {@link TypingStatisticsModel.Section} 无关，那是旧分布图的枚举，设备测试 APK 仍编译它。 */
    private enum Tab {
        OVERVIEW("概览"), HABITS("习惯"), KEYS("按键"), ACHIEVEMENTS("成就");

        final String label;

        Tab(String label) {
            this.label = label;
        }
    }

    /** 一次读取的结果：统计文档（开关、保留期、逐日按键）和派生指标。两者都可能为 null。 */
    private record Snapshot(@Nullable TypingStatisticsModel model, @Nullable TypingStatisticsSummary summary) {}

    private static final Map<String, String> RETENTIONS = retentions();
    private static final int WEEK = 7;

    private Tab tab = Tab.OVERVIEW;
    @Nullable private TypingStatisticsModel statistics;
    @Nullable private TypingStatisticsSummary summary;
    /** 按键热力图上次选的布局；null 表示还没选过，按数据决定。 */
    @Nullable private Boolean nineKey;

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_statistics, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        SegmentedControl sections = view.findViewById(R.id.statistics_sections);
        sections.setFillWidth(true);
        List<String> labels = new ArrayList<>(Tab.values().length);
        for (Tab value : Tab.values()) labels.add(value.label);
        sections.setOptions(labels, tab.ordinal());
        sections.setOnSelect(index -> {
            tab = Tab.values()[index];
            render();
        });
        view.findViewById(R.id.statistics_menu).setOnClickListener(this::showMenu);
        TextView footer = view.findViewById(R.id.statistics_footer);
        Drawable lock = ContextCompat.getDrawable(requireContext(), R.drawable.ic_ms_shield_lock);
        if (lock != null) {
            lock = lock.mutate();
            int size = Ui.dp(requireContext(), 14);
            lock.setBounds(0, 0, size, size);
            lock.setTint(Ui.subText(requireContext()));
            footer.setCompoundDrawablesRelative(lock, null, null, null);
        }
        reload();
    }

    /** 记录开关、保留期、刷新和清空：偶尔才改一次，收在菜单里，不占读数字的地方。 */
    private void showMenu(View anchor) {
        TypingStatisticsModel current = statistics;
        if (current == null) return;
        PopupMenu menu = new PopupMenu(requireContext(), anchor);
        MenuItem record = menu.getMenu().add("记录打字统计");
        record.setCheckable(true);
        record.setChecked(current.enabled());
        record.setOnMenuItemClickListener(item -> {
            boolean next = !current.enabled();
            HostTask.run(this, context -> HostStore.setStatisticsEnabled(context, next), this::adoptModel);
            return true;
        });
        SubMenu retention = menu.getMenu().addSubMenu("保留每日明细");
        for (Map.Entry<String, String> entry : RETENTIONS.entrySet()) {
            String key = entry.getKey();
            MenuItem item = retention.add(entry.getValue());
            item.setCheckable(true);
            item.setChecked(key.equals(current.retention()));
            item.setOnMenuItemClickListener(ignored -> {
                HostTask.run(this, context -> HostStore.setStatisticsRetention(context, key),
                    value -> reload());
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
                .setMessage("今日、累计、全部分类、按键计数和已解锁的成就都会清空，且无法恢复。键盘会从下一次输入重新开始记录。")
                .setNegativeButton("取消", null)
                .setPositiveButton("清除", (dialog, which) -> HostTask.run(this,
                    context -> HostStore.resetStatistics(context), value -> reload()))
                .show();
            return true;
        });
        menu.show();
    }

    // 键盘在另一个进程里，这个页面不在眼前时它一直在记。
    @Override protected void onBecameVisible() {
        if (statistics != null) reload();
    }

    private void reload() {
        HostTask.run(this, StatisticsFragment::load, this::adopt);
    }

    /** 工作线程上：读统计文档，取用户词条数，再取派生指标。 */
    private static Snapshot load(Context context) {
        TypingStatisticsModel model = HostStore.loadStatistics(context);
        JSONObject action = new JSONObject();
        try {
            action.put("operation", "summary").put("day", LocalDate.now().toString());
            Long words = userWords(context);
            if (words != null) action.put("user_words", words.longValue());
        } catch (JSONException error) {
            return new Snapshot(model, null);
        }
        return new Snapshot(model, TypingStatisticsSummary.from(HostStore.statisticsAction(context, action)));
    }

    /** 用户自己添加的词条数（徽章「造词者」）；词库读不到时返回 null，summary 按 0 计。 */
    @Nullable private static Long userWords(Context context) {
        File options = new File(context.getFilesDir(), "runtime-options.json");
        if (!options.isFile()) return null;
        try {
            JSONObject request = new JSONObject()
                .put("options", new JSONObject(HostOptionsPolicy.read(options)))
                .put("action", new JSONObject().put("operation", "count").put("kind", "pinyin")
                    .put("user_only", true));
            JSONObject root = new JSONObject(NativeClient.dictionary(request.toString()));
            if (!root.optBoolean("ok", false)) return null;
            JSONObject value = root.optJSONObject("value");
            return value == null || !value.has("count") ? null : BoundsPolicy.nonNegative(value.optLong("count", 0));
        } catch (JSONException | java.io.IOException | RuntimeException | LinkageError error) {
            return null;
        }
    }

    private void adopt(@Nullable Snapshot snapshot) {
        if (snapshot != null) {
            if (snapshot.model() != null) statistics = snapshot.model();
            if (snapshot.summary() != null) summary = snapshot.summary();
        }
        render();
    }

    private void adoptModel(@Nullable TypingStatisticsModel value) {
        if (value != null) statistics = value;
        render();
    }

    private void render() {
        View view = getView();
        if (view == null) return;
        Context context = requireContext();
        TextView notice = view.findViewById(R.id.statistics_state);
        LinearLayout content = view.findViewById(R.id.statistics_content);
        content.removeAllViews();
        if (statistics == null && summary == null) {
            notice.setVisibility(View.VISIBLE);
            notice.setText("还没有记录。开始用键盘输入后，这里会出现字数、习惯和成就；统计只保存聚合计数，不保存输入内容。");
            return;
        }
        boolean off = statistics != null && !statistics.enabled();
        notice.setVisibility(off || summary == null ? View.VISIBLE : View.GONE);
        notice.setText(off ? "记录已关闭。已有的计数保留在本机，新的输入不再计入。可以在右上角菜单里打开。"
            : "统计暂时读不到，可以在右上角菜单里刷新。");
        if (summary == null) return;
        switch (tab) {
            case OVERVIEW -> overview(context, content, summary.overview());
            case HABITS -> habits(context, content, summary.habits());
            case KEYS -> keys(context, content, summary);
            case ACHIEVEMENTS -> achievements(context, content, summary.achievements());
        }
    }

    // ---- 概览 ----

    private void overview(Context context, LinearLayout content, Overview overview) {
        LinearLayout hero = card(context, content, 18);
        hero.addView(label(context, "近 7 天共输入", 13, Ui.subText(context)));
        TextView total = new TextView(context);
        total.setText(figure(context, TypingStatisticsSummary.grouped(overview.weekTotal()), 40, "字"));
        Ui.setPaddingDp(total, context, 0, 4, 0, 0);
        hero.addView(total);
        String delta = TypingStatisticsSummary.weekDelta(overview.weekTotal(), overview.previousWeekTotal());
        if (delta != null) {
            TextView change = label(context, delta, 13, Ui.accent(context));
            change.setTypeface(Typeface.DEFAULT_BOLD);
        Ui.setPaddingDp(change, context, 0, 4, 0, 0);
            hero.addView(change);
        }
        TrendChart chart = new TrendChart(context);
        chart.setDays(overview.last7());
        LinearLayout.LayoutParams chartParams = Ui.matchWidth();
        chartParams.topMargin = Ui.dp(context, 18);
        hero.addView(chart, chartParams);

        String speedNote = TypingStatisticsSummary.speedDelta(overview.averageSpeed(),
            overview.previousAverageSpeed());
        tiles(context, content,
            tile(context, "平均速度", TypingStatisticsSummary.whole(overview.averageSpeed()), "字/分",
                speedNote == null ? "近 7 天的活跃时间里" : speedNote, false),
            tile(context, "首选命中", TypingStatisticsSummary.percent(overview.firstCandidateRate()), "%",
                overview.firstCandidateRate() == null ? "选词满 50 次后显示" : "第一个候选就是你要的", false));
        tiles(context, content,
            tile(context, "少按键", TypingStatisticsSummary.percent(overview.keystrokesSavedRate()), "%",
                "联想和整句帮你省下", false),
            tile(context, "连续使用", String.valueOf(overview.currentStreak()), "天",
                "最长 " + overview.longestStreak() + " 天", false));
    }

    // ---- 习惯 ----

    private void habits(Context context, LinearLayout content, Habits habits) {
        header(context, content, "近 12 周", "活跃 " + habits.activeDays() + " 天");
        LinearLayout heat = card(context, content, 16);
        HeatmapView heatmap = new HeatmapView(context);
        heatmap.setDays(habits.weeks12());
        heat.addView(heatmap, Ui.matchWidth());

        String peak = TypingStatisticsSummary.peakLabel(habits.peakWindow());
        header(context, content, "活跃时段", peak == null ? null : "最常在 " + peak);
        LinearLayout hours = card(context, content, 16);
        HourHistogramView histogram = new HourHistogramView(context);
        histogram.setHours(habits.hours24(), habits.peakWindow(), peak);
        hours.addView(histogram, Ui.matchWidth());

        List<Share> mix = TypingStatisticsSummary.composition(habits.characters());
        header(context, content, "输入构成", null);
        LinearLayout composition = card(context, content, 16);
        if (mix.isEmpty()) {
            composition.addView(label(context, "还没有记录", 14, Ui.subText(context)));
        } else {
            DistributionView bar = new DistributionView(context);
            bar.setShares(mix, DistributionView.Style.STACK);
            composition.addView(bar, Ui.matchWidth());
        }
    }

    // ---- 按键 ----

    private void keys(Context context, LinearLayout content, TypingStatisticsSummary value) {
        Map<String, Long> presses = weekKeys();
        boolean nine = nineKey != null ? nineKey : KeyHeatmapView.prefersNine(presses);
        LinearLayout row = header(context, content, "按键热力图", null);
        SegmentedControl layout = new SegmentedControl(context);
        layout.setOptions(List.of("26 键", "9 键"), nine ? 1 : 0);
        layout.setMinimumHeight(Ui.dp(context, 32));
        row.addView(layout, Ui.wrapHeight(context, 32));
        LinearLayout board = card(context, content, 12);
        KeyHeatmapView heatmap = new KeyHeatmapView(context);
        heatmap.setNineKey(nine);
        heatmap.setKeys(presses);
        board.addView(heatmap, Ui.matchWidth());
        layout.setOnSelect(index -> {
            nineKey = index == 1;
            heatmap.setNineKey(nineKey);
        });

        Keys keys = value.keys();
        String perKeyNote = TypingStatisticsSummary.perKeyDelta(keys.perCharacterKeys(),
            keys.previousPerCharacterKeys());
        tiles(context, content,
            tile(context, "每字按键", TypingStatisticsSummary.decimal(keys.perCharacterKeys()), "次",
                perKeyNote == null ? "近 7 天平均" : perKeyNote, perKeyNote != null),
            tile(context, "退格占比", TypingStatisticsSummary.percentTenths(keys.backspaceRate()), "%",
                "近 7 天全部按键里", false));
        String runNote = keys.longestRun() == null ? "还没有记录"
            : TypingStatisticsSummary.monthDay(keys.longestRun().day()) + " · 不停顿";
        tiles(context, content,
            tile(context, "联想上屏", TypingStatisticsSummary.percent(keys.predictionRate()), "%",
                "不用打完就上屏的词", false),
            tile(context, "单次最长", keys.longestRun() == null ? "—"
                : String.valueOf(keys.longestRun().characters()), "字", runNote, false));

        if (keys.positions() != null) {
            header(context, content, "选词位置", null);
            LinearLayout positions = card(context, content, 16);
            List<Share> shares = new ArrayList<>(TypingStatisticsSummary.POSITION_BUCKETS);
            String[] titles = {"第 1 个", "第 2 个", "第 3 个", "翻页后"};
            for (int index = 0; index < TypingStatisticsSummary.POSITION_BUCKETS; index++) {
                shares.add(new Share(titles[index], Math.round(keys.positions().get(index) * 1000)));
            }
            DistributionView bars = new DistributionView(context);
            bars.setShares(shares, DistributionView.Style.BARS);
            positions.addView(bars, Ui.matchWidth());
        }

        List<Share> methods = TypingStatisticsSummary.methods(value.habits().sources());
        if (!methods.isEmpty()) {
            header(context, content, "输入方式", null);
            LinearLayout card = card(context, content, 16);
            DistributionView donut = new DistributionView(context);
            donut.setShares(methods, DistributionView.Style.DONUT);
            card.addView(donut, Ui.matchWidth());
        }
    }

    /** 近 7 天每个键的按键次数，和按键 KPI 同一个时间窗。 */
    private Map<String, Long> weekKeys() {
        Map<String, Long> result = new LinkedHashMap<>(KeyPressIds.KEY_IDS.size());
        if (statistics == null) return result;
        LocalDate today = LocalDate.now();
        for (int offset = 0; offset < WEEK; offset++) {
            for (Map.Entry<String, Long> entry : statistics.keys(today.minusDays(offset).toString()).entrySet()) {
                result.merge(entry.getKey(), entry.getValue(), Long::sum);
            }
        }
        return result;
    }

    // ---- 成就 ----

    private void achievements(Context context, LinearLayout content, List<Achievement> badges) {
        int unlocked = 0;
        for (Achievement badge : badges) if (badge.unlocked()) unlocked++;
        LinearLayout progress = card(context, content, 16);
        TextView count = new TextView(context);
        SpannableStringBuilder text = new SpannableStringBuilder(String.valueOf(unlocked));
        text.setSpan(new AbsoluteSizeSpan(26, true), 0, text.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        text.setSpan(new android.text.style.StyleSpan(Typeface.BOLD), 0, text.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        int start = text.length();
        text.append(" / ").append(String.valueOf(badges.size())).append(" 已解锁");
        text.setSpan(new AbsoluteSizeSpan(15, true), start, text.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        text.setSpan(new ForegroundColorSpan(Ui.subText(context)), start, text.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        count.setText(text);
        count.setTextColor(Ui.text(context));
        progress.addView(count);
        View track = new View(context);
        track.setBackground(Ui.pill(Ui.hairline(context)));
        LinearLayout.LayoutParams trackParams = Ui.matchWidthHeight(context, 6);
        trackParams.topMargin = Ui.dp(context, 12);
        android.widget.FrameLayout bar = new android.widget.FrameLayout(context);
        bar.addView(track, new android.widget.FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, Ui.dp(context, 6)));
        View fillView = new View(context);
        fillView.setBackground(Ui.pill(Ui.accent(context)));
        bar.addView(fillView, new android.widget.FrameLayout.LayoutParams(0, Ui.dp(context, 6)));
        progress.addView(bar, trackParams);
        float share = badges.isEmpty() ? 0 : unlocked / (float) badges.size();
        bar.post(() -> {
            ViewGroup.LayoutParams params = fillView.getLayoutParams();
            params.width = Math.round(bar.getWidth() * share);
            fillView.setLayoutParams(params);
        });
        progress.setContentDescription(unlocked + " / " + badges.size() + " 枚成就已解锁");

        BadgeGridView grid = new BadgeGridView(context);
        grid.setBadges(badges);
        grid.setOnBadgeTap(badge -> MsToast.show(context, TypingStatisticsSummary.toast(badge)));
        LinearLayout.LayoutParams gridParams = Ui.matchWidth();
        gridParams.topMargin = Ui.dp(context, 10);
        content.addView(grid, gridParams);
    }

    // ---- 搭卡片的小工具 ----

    /** 一张统计卡：andCard 底、20dp 圆角，加在 `parent` 末尾。 */
    private static LinearLayout card(Context context, LinearLayout parent, int padding) {
        LinearLayout card = new LinearLayout(context);
        card.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(context, padding);
        Ui.setSymmetricPaddingPx(card, pad);
        card.setBackground(Ui.rounded(Ui.card(context), Ui.dp(context, 20)));
        LinearLayout.LayoutParams params = Ui.matchWidth();
        params.topMargin = Ui.dp(context, parent.getChildCount() == 0 ? 16 : 10);
        parent.addView(card, params);
        return card;
    }

    /** 卡片上方的一行：左边小标题，右边可选的说明；返回这一行，按键页往右边再放分段控件。 */
    private static LinearLayout header(Context context, LinearLayout parent, String title,
            @Nullable String trailing) {
        LinearLayout row = new LinearLayout(context);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        Ui.setHorizontalPaddingDp(row, context, 4);
        TextView heading = label(context, title, 13, Ui.subText(context));
        heading.setAccessibilityHeading(true);
        row.addView(heading, Ui.weightWrap(1f));
        if (trailing != null) row.addView(label(context, trailing, 13, Ui.subText(context)));
        LinearLayout.LayoutParams params = Ui.matchWidth();
        params.topMargin = Ui.dp(context, 22);
        // 最小 32 dp 而不是固定 32 dp：系统字体调大后标题和右侧的分段控件都比它高。
        row.setMinimumHeight(Ui.dp(context, 32));
        parent.addView(row, params);
        return row;
    }

    /** 并排两张 KPI 卡。 */
    private static void tiles(Context context, LinearLayout parent, View left, View right) {
        LinearLayout row = new LinearLayout(context);
        row.setOrientation(LinearLayout.HORIZONTAL);
        LinearLayout.LayoutParams leftParams = Ui.weightedMatchParent(1f);
        LinearLayout.LayoutParams rightParams = Ui.weightedMatchParent(1f);
        rightParams.setMarginStart(Ui.dp(context, 10));
        row.addView(left, leftParams);
        row.addView(right, rightParams);
        LinearLayout.LayoutParams params = Ui.matchWidth();
        params.topMargin = Ui.dp(context, 10);
        parent.addView(row, params);
    }

    /** 一张 KPI 卡：标题、大数字和单位、一行说明；`highlight` 时说明用 accent（环比）。 */
    private static View tile(Context context, String title, String value, String unit, String note,
            boolean highlight) {
        LinearLayout tile = new LinearLayout(context);
        tile.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(context, 14);
        Ui.setSymmetricPaddingPx(tile, pad);
        tile.setBackground(Ui.rounded(Ui.card(context), Ui.dp(context, 20)));
        tile.addView(label(context, title, 13, Ui.text(context)));
        TextView number = new TextView(context);
        number.setText(figure(context, value, 24, "—".equals(value) ? "" : unit));
        Ui.setPaddingDp(number, context, 0, 6, 0, 6);
        tile.addView(number);
        tile.addView(label(context, note, 12, highlight ? Ui.accent(context) : Ui.subText(context)));
        tile.setContentDescription(title + " " + value + ("—".equals(value) ? "" : " " + unit) + "，" + note);
        tile.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
        return tile;
    }

    /** 大数字加小号单位：`12,846 字`。 */
    private static CharSequence figure(Context context, String value, int sizeSp, String unit) {
        SpannableStringBuilder text = new SpannableStringBuilder(value);
        text.setSpan(new AbsoluteSizeSpan(sizeSp, true), 0, value.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        text.setSpan(new android.text.style.StyleSpan(Typeface.BOLD), 0, value.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        text.setSpan(new ForegroundColorSpan(Ui.text(context)), 0, value.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        if (!unit.isEmpty()) {
            int start = text.length();
            text.append(' ').append(unit);
            text.setSpan(new AbsoluteSizeSpan(
                BoundsPolicy.bounded(sizeSp / 3, 12, Integer.MAX_VALUE), true), start, text.length(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
            text.setSpan(new ForegroundColorSpan(Ui.text(context)), start, text.length(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        return text;
    }

    private static TextView label(Context context, String text, int sizeSp, int colour) {
        return Ui.label(context, text, sizeSp, colour);
    }

    private static Map<String, String> retentions() {
        LinkedHashMap<String, String> values = new LinkedHashMap<>(5);
        values.put("forever", "一直保留");
        values.put("365d", "一年");
        values.put("180d", "半年");
        values.put("90d", "90 天");
        values.put("30d", "30 天");
        return java.util.Collections.unmodifiableMap(values);
    }
}
