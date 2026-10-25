package app.msime.android.home;

import app.msime.android.DimensionPolicy;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.LayoutPolicy;

import android.app.KeyguardManager;
import android.content.Context;
import android.graphics.Typeface;
import android.graphics.drawable.Drawable;
import android.os.Bundle;
import android.text.SpannableStringBuilder;
import android.text.Spanned;
import android.text.style.AbsoluteSizeSpan;
import android.text.style.ForegroundColorSpan;
import android.view.LayoutInflater;
import android.view.MenuItem;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import androidx.biometric.BiometricManager;
import androidx.biometric.BiometricPrompt;
import app.msime.android.NativeClient;
import app.msime.android.StatisticsAuthenticationPolicy;
import app.msime.android.SystemAuthenticators;
import app.msime.android.AuthenticationPauseTracker;
import app.msime.android.DrawablePolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.BoundsPolicy;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.KeyHeatmapLayout;
import app.msime.android.KeyPressIds;
import app.msime.android.KeyboardGeometry;
import app.msime.android.NumberPolicy;
import app.msime.android.R;
import app.msime.android.TypingStatisticsModel;
import app.msime.android.TypingStatisticsSummary;
import app.msime.android.TypingStatisticsSummary.Achievement;
import app.msime.android.TypingStatisticsSummary.Habits;
import app.msime.android.TypingStatisticsSummary.Keys;
import app.msime.android.TypingStatisticsSummary.Overview;
import app.msime.android.TypingStatisticsSummary.Share;
import app.msime.android.TypingUploadApi;
import app.msime.android.ViewPolicy;
import app.msime.android.policy.HostOptionsPolicy;
import java.time.LocalDate;
import java.time.ZoneId;
import java.util.ArrayList;
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

    private Tab tab = Tab.OVERVIEW;
    @Nullable private TypingStatisticsModel statistics;
    @Nullable private TypingStatisticsSummary summary;
    /** 「上传到云端」的状态；还没读到时为 null，概览页不画这一组。 */
    @Nullable private TypingUpload.Status upload;
    private boolean uploadBusy;
    /** 按键热力图上次选的布局；null 表示还没选过，按数据决定。 */
    @Nullable private Boolean nineKey;
    private boolean authorized;
    @Nullable private BiometricPrompt authentication;
    private final AuthenticationPauseTracker pause = new AuthenticationPauseTracker();

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
        View menu = view.findViewById(R.id.statistics_menu);
        ViewPolicy.bindClick(menu, () -> showMenu(menu));
        TextView footer = view.findViewById(R.id.statistics_footer);
        Drawable lock = ContextCompat.getDrawable(requireContext(), R.drawable.ic_ms_shield_lock);
        if (lock != null) {
            lock = lock.mutate();
            int size = DimensionPolicy.pixels(requireContext(), 14);
            lock.setBounds(0, 0, size, size);
            lock.setTint(ThemeColorPolicy.subText(requireContext()));
            footer.setCompoundDrawablesRelative(lock, null, null, null);
        }
        requestAuthentication();
    }

    @Override public void onDestroyView() {
        if (authentication != null) authentication.cancelAuthentication();
        authentication = null;
        authorized = false;
        pause.reset();
        super.onDestroyView();
    }

    private void requestAuthentication() {
        if (authentication != null) return;
        Context context = requireContext();
        KeyguardManager keyguard = (KeyguardManager) context.getSystemService(Context.KEYGUARD_SERVICE);
        if (keyguard == null) {
            MsToast.show(context, "无法读取系统认证状态，请稍后重试");
            return;
        }
        // 未设置锁屏凭据时直接读统计；已配置认证的设备仍需认证成功，临时不可用不算未配置。
        if (!StatisticsAuthenticationPolicy.requiresAuthentication(authorized, keyguard.isDeviceSecure())) {
            reload();
            return;
        }
        int authenticators = SystemAuthenticators.allowed();
        if (BiometricManager.from(context).canAuthenticate(authenticators)
                != BiometricManager.BIOMETRIC_SUCCESS) {
            MsToast.show(context, "这台设备没有可用的系统认证");
            return;
        }
        authentication = new BiometricPrompt(this, ContextCompat.getMainExecutor(context),
            new BiometricPrompt.AuthenticationCallback() {
                @Override public void onAuthenticationSucceeded(BiometricPrompt.AuthenticationResult result) {
                    authentication = null;
                    authorized = true;
                    if (getView() != null) reload();
                    authenticationEnded(false);
                }

                @Override public void onAuthenticationError(int errorCode, CharSequence errorString) {
                    authentication = null;
                    authorized = false;
                    authenticationEnded(AuthenticationPauseTracker.systemCanceled(errorCode, android.os.Build.VERSION.SDK_INT));
                }
            });
        BiometricPrompt.PromptInfo prompt = new BiometricPrompt.PromptInfo.Builder()
            .setTitle("验证身份")
            .setSubtitle("统计页面包含本机输入习惯数据")
            .setAllowedAuthenticators(authenticators)
            .build();
        authentication.authenticate(prompt);
    }

    /** 记录开关、刷新和清空：偶尔才改一次，收在菜单里，不占读数字的地方。每日明细的保留期不在这里改，已存的值照旧生效。 */
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
        menu.getMenu().add("刷新统计").setOnMenuItemClickListener(item -> {
            reload();
            return true;
        });
        menu.getMenu().add("清空统计").setOnMenuItemClickListener(item -> {
            MessageDialog.confirm(requireContext(), "清除全部统计",
                "今日、累计、全部分类、按键计数和已解锁的成就都会清空，且无法恢复。键盘会从下一次输入重新开始记录。",
                "清除", () -> HostTask.run(this, context -> HostStore.resetStatistics(context), value -> reload()));
            return true;
        });
        menu.show();
    }

    @Override public void onPause() {
        super.onPause();
        pause.paused(authentication != null);
    }

    // 键盘在另一个进程里，这个页面不在眼前时它一直在记，所以每次重新可见都重新认证再读一次。例外是暂停时认证还开着：回来时是否重新认证由这次认证的结果决定，见 AuthenticationPauseTracker；认证成功的回调自己会读。
    @Override protected void onBecameVisible() {
        if (pause.reentersOnVisible(authentication != null)) reenter();
    }

    private void reenter() {
        authorized = false;
        requestAuthentication();
    }

    /** 认证在离开页面时被系统取消、而页面已经回到眼前：按重新进入处理，否则统计页既没有数据也没有认证窗口。 */
    private void authenticationEnded(boolean systemCanceled) {
        if (pause.reentersOnEnd(systemCanceled, isResumed() && !isHidden() && getView() != null)) reenter();
    }

    private void reload() {
        HostTask.run(this, StatisticsFragment::load, this::adopt);
        // 打开统计页时补传一次（与共享设置页相同），顺带读出开关；要联网，不排在读统计的队列里。
        HostTask.runNetwork(this, TypingUpload::sync, this::adoptUpload);
    }

    private void adoptUpload(@Nullable TypingUpload.Status value) {
        if (value != null) upload = value;
        render();
    }

    /** 打开立刻上传一次，关闭先在本机关掉再删除云端的数据；结果用一句话告诉用户。 */
    private void setUpload(boolean enabled) {
        if (uploadBusy) return;
        uploadBusy = true;
        render();
        HostTask.runNetwork(this, context -> TypingUpload.setEnabled(context, enabled), change -> {
            uploadBusy = false;
            if (change == null) {
                MsToast.show(requireContext(), "无法读取或保存上传设置，请稍后重试。");
                reload();
                return;
            }
            upload = change.status();
            MsToast.show(requireContext(), change.message());
            render();
        });
    }

    /** 概览页最后一组：「上传到云端」。没登录真实账号时不能打开，已经打开的仍能关掉。 */
    private void upload(Context context, LinearLayout content) {
        TypingUpload.Status current = upload;
        if (current == null) return;
        boolean usable = current.signedIn() || current.enabled();
        GroupCard group = GroupCard.add(content, "云端");
        GroupCard.Row row = group.toggle("上传到云端", usable
                ? "把每天的字数、活跃时长、时段、输入方式和候选位次上传到你的水杉账号，供 QQ 群助手的 /stats 查看。只有计数，不含输入内容；关闭时删除云端的数据。"
                : "登录水杉账号后可以打开，供 QQ 群助手的 /stats 查看。只有计数，不含输入内容。",
            current.enabled(), this::setUpload);
        row.setEnabled(usable && !uploadBusy);
        Long last = current.lastUploadMs();
        if (current.enabled() && last != null) {
            group.footer("上次上传：" + TypingUploadApi.displayMillis(last, ZoneId.systemDefault()));
        }
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
        try {
            JSONObject request = new JSONObject()
                .put("options", new JSONObject(HostOptionsPolicy.readRuntimeOptions(context.getFilesDir())))
                .put("action", new JSONObject().put("operation", "count").put("kind", "pinyin")
                    .put("user_only", true));
            JSONObject root = new JSONObject(NativeClient.dictionary(request.toString()));
            if (!JsonPolicy.strictTrue(root.opt("ok"))) return null;
            JSONObject value = JsonPolicy.objectValue(root);
            if (value == null || !value.has("count")) return null;
            Long count = JsonPolicy.strictLong(value.opt("count"));
            return count == null ? null : BoundsPolicy.nonNegative(count);
        } catch (JSONException | RuntimeException | LinkageError error) {
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
        // 布局里写的是「只保存在本机」；打开了上传时改写，不能让这句话骗人。
        TextView footer = view.findViewById(R.id.statistics_footer);
        footer.setText(upload != null && upload.enabled() ? "已打开「上传到云端」：每日计数会上传到你的水杉账号，不含输入内容"
            : "统计只保存在本机，不包含输入内容");
        TextView notice = view.findViewById(R.id.statistics_state);
        LinearLayout content = view.findViewById(R.id.statistics_content);
        content.removeAllViews();
        if (statistics == null && summary == null) {
            ViewPolicy.show(notice);
            notice.setText("统计记录还没有开启。开启后才会从下一次输入开始累计；只保存聚合计数，不保存输入内容。");
            GroupCard setup = GroupCard.add(content, "统计记录");
            setup.note("统计默认关闭，所以空页面不是故障。你可以随时关闭，已有数据会保留在本机。");
            setup.button("记录打字统计", "从下一次输入开始累计", "开启", () ->
                HostTask.run(this, ctx -> HostStore.setStatisticsEnabled(ctx, true), this::adoptModel));
            return;
        }
        boolean off = statistics != null && !statistics.enabled();
        ViewPolicy.setVisible(notice, off || summary == null);
        notice.setText(off ? "记录已关闭。已有的计数保留在本机，新的输入不再计入。可以在右上角菜单里打开。"
            : "统计暂时读不到，可以在右上角菜单里刷新。");
        if (summary == null) return;
        switch (tab) {
            case OVERVIEW -> {
                overview(context, content, summary.overview());
                upload(context, content);
            }
            case HABITS -> habits(context, content, summary.habits());
            case KEYS -> keys(context, content, summary);
            case ACHIEVEMENTS -> achievements(context, content, summary.achievements());
        }
    }

    // ---- 概览 ----

    private void overview(Context context, LinearLayout content, Overview overview) {
        LinearLayout hero = card(context, content, 18);
        hero.addView(ViewPolicy.label(context, "近 7 天共输入", 13, ThemeColorPolicy.subText(context)));
        TextView total = ViewPolicy.label(context,
            figure(context, NumberPolicy.grouped(overview.weekTotal()), 40, "字"),
            40, ThemeColorPolicy.text(context));
        ViewPolicy.setPaddingDp(context, total, 0, 4, 0, 0);
        hero.addView(total);
        String delta = TypingStatisticsSummary.weekDelta(overview.weekTotal(), overview.previousWeekTotal());
        if (delta != null) {
            TextView change = ViewPolicy.label(context, delta, 13, ThemeColorPolicy.accent(context));
            ViewPolicy.setTypefaceStyle(change, Typeface.BOLD);
        ViewPolicy.setPaddingDp(context, change, 0, 4, 0, 0);
            hero.addView(change);
        }
        TrendChart chart = new TrendChart(context);
        chart.setDays(overview.last7());
        LinearLayout.LayoutParams chartParams = LayoutPolicy.matchWidthWrapParams();
        chartParams.topMargin = DimensionPolicy.pixels(context, 18);
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
        heat.addView(heatmap, LayoutPolicy.matchWidthWrapParams());

        String peak = TypingStatisticsSummary.peakLabel(habits.peakWindow());
        header(context, content, "活跃时段", peak == null ? null : "最常在 " + peak);
        LinearLayout hours = card(context, content, 16);
        HourHistogramView histogram = new HourHistogramView(context);
        histogram.setHours(habits.hours24(), habits.peakWindow(), peak);
        hours.addView(histogram, LayoutPolicy.matchWidthWrapParams());

        List<Share> mix = TypingStatisticsSummary.composition(habits.characters());
        header(context, content, "输入构成", null);
        LinearLayout composition = card(context, content, 16);
        if (mix.isEmpty()) {
            composition.addView(ViewPolicy.label(context, "还没有记录", 14, ThemeColorPolicy.subText(context)));
        } else {
            DistributionView bar = new DistributionView(context);
            bar.setShares(mix, DistributionView.Style.STACK);
            composition.addView(bar, LayoutPolicy.matchWidthWrapParams());
        }
    }

    // ---- 按键 ----

    private void keys(Context context, LinearLayout content, TypingStatisticsSummary value) {
        Map<String, Long> presses = weekKeys();
        boolean nine = nineKey != null ? nineKey : KeyHeatmapView.prefersNine(presses);
        LinearLayout row = header(context, content, "按键热力图", null);
        SegmentedControl layout = new SegmentedControl(context);
        layout.setOptions(List.of("26 键", "9 键"), nine ? 1 : 0);
        ViewPolicy.setMinimumHeight(layout, DimensionPolicy.pixels(context, 32));
        row.addView(layout, LayoutPolicy.linearParamsPx(
            ViewGroup.LayoutParams.WRAP_CONTENT, DimensionPolicy.pixels(context, 32)));
        LinearLayout board = card(context, content, 12);
        KeyHeatmapView heatmap = new KeyHeatmapView(context);
        heatmap.setNineKey(nine);
        heatmap.setKeys(presses);
        board.addView(heatmap, LayoutPolicy.matchWidthWrapParams());
        LinearLayout others = card(context, content, 16);
        otherKeys(context, others, presses, nine);
        layout.setOnSelect(index -> {
            nineKey = index == 1;
            heatmap.setNineKey(nineKey);
            otherKeys(context, others, presses, nineKey);
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
            positions.addView(bars, LayoutPolicy.matchWidthWrapParams());
        }

        List<Share> methods = TypingStatisticsSummary.methods(value.habits().sources());
        if (!methods.isEmpty()) {
            header(context, content, "输入方式", null);
            LinearLayout card = card(context, content, 16);
            DistributionView donut = new DistributionView(context);
            donut.setShares(methods, DistributionView.Style.DONUT);
            card.addView(donut, LayoutPolicy.matchWidthWrapParams());
        }
    }

    /**
     * 热力图下的「其他键」卡：按过、但当前这套键盘图上没有位置的键（{@link KeyHeatmapLayout#others}），按次数从多到少，每行键名和次数，让每一次按键都能在页面上找到。没有这样的键时整张卡不显示；切换 26 键 / 9 键时重新列。
     */
    private static void otherKeys(Context context, LinearLayout card, Map<String, Long> presses, boolean nine) {
        card.removeAllViews();
        List<String> ids = KeyHeatmapLayout.others(presses, nine);
        ViewPolicy.setVisible(card, !ids.isEmpty());
        if (ids.isEmpty()) return;
        card.addView(ViewPolicy.label(context, "其他键", 15, ThemeColorPolicy.text(context)));
        for (String id : ids) {
            String label = KeyPressIds.label(id);
            String count = NumberPolicy.grouped(presses.get(id)) + " 次";
            LinearLayout row = LayoutPolicy.row(context);
            ViewPolicy.setCenteredVertically(row);
            ViewPolicy.setMinimumHeightDp(context, row, 36);
            row.addView(ViewPolicy.label(context, label, 15, ThemeColorPolicy.text(context)),
                LayoutPolicy.weightedWrapParams(1f));
            row.addView(ViewPolicy.label(context, count, 15, ThemeColorPolicy.subText(context)));
            row.setContentDescription(label + " " + count);
            ViewPolicy.setImportantForAccessibility(row, View.IMPORTANT_FOR_ACCESSIBILITY_YES);
            card.addView(row, LayoutPolicy.matchWidthWrapParams());
        }
        TextView note = ViewPolicy.label(context, "数字、标点、符号键等键盘图上没有位置的键。", 12,
            ThemeColorPolicy.subText(context));
        ViewPolicy.setPaddingDp(context, note, 0, 6, 0, 0);
        card.addView(note);
    }

    /** 近 7 天每个键的按键次数，和按键 KPI 同一个时间窗。 */
    private Map<String, Long> weekKeys() {
        if (statistics == null || summary == null) return Map.of();
        List<String> days = new ArrayList<>(summary.overview().last7().size());
        for (TypingStatisticsSummary.DayCount day : summary.overview().last7()) days.add(day.day());
        return statistics.keysForDays(days);
    }

    // ---- 成就 ----

    private void achievements(Context context, LinearLayout content, List<Achievement> badges) {
        int unlocked = 0;
        for (Achievement badge : badges) if (badge.unlocked()) unlocked++;
        LinearLayout progress = card(context, content, 16);
        SpannableStringBuilder text = new SpannableStringBuilder(String.valueOf(unlocked));
        text.setSpan(new AbsoluteSizeSpan(26, true), 0, text.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        text.setSpan(new android.text.style.StyleSpan(Typeface.BOLD), 0, text.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        int start = text.length();
        text.append(" / ").append(String.valueOf(badges.size())).append(" 已解锁");
        text.setSpan(new AbsoluteSizeSpan(15, true), start, text.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        text.setSpan(new ForegroundColorSpan(ThemeColorPolicy.subText(context)), start, text.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        TextView count = ViewPolicy.label(context, text, 15, ThemeColorPolicy.text(context));
        progress.addView(count);
        View track = new View(context);
        track.setBackground(DrawablePolicy.pill(ThemeColorPolicy.hairline(context)));
        LinearLayout.LayoutParams trackParams = LayoutPolicy.matchWidthHeightDp(context, 6);
        trackParams.topMargin = DimensionPolicy.pixels(context, 12);
        android.widget.FrameLayout bar = new android.widget.FrameLayout(context);
        bar.addView(track, new android.widget.FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, DimensionPolicy.pixels(context, 6)));
        View fillView = new View(context);
        fillView.setBackground(DrawablePolicy.pill(ThemeColorPolicy.accent(context)));
        bar.addView(fillView, new android.widget.FrameLayout.LayoutParams(0, DimensionPolicy.pixels(context, 6)));
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
        LinearLayout.LayoutParams gridParams = LayoutPolicy.matchWidthWrapParams();
        gridParams.topMargin = DimensionPolicy.pixels(context, 10);
        content.addView(grid, gridParams);
    }

    // ---- 搭卡片的小工具 ----

    /** 一张统计卡：andCard 底、20dp 圆角，加在 `parent` 末尾。 */
    private static LinearLayout card(Context context, LinearLayout parent, int padding) {
        LinearLayout card = LayoutPolicy.roundedColumn(context, ThemeColorPolicy.card(context), 20);
        int pad = DimensionPolicy.pixels(context, padding);
        ViewPolicy.setSymmetricPadding(card, pad);
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthWrapParams();
        params.topMargin = DimensionPolicy.pixels(context, parent.getChildCount() == 0 ? 16 : 10);
        parent.addView(card, params);
        return card;
    }

    /** 卡片上方的一行：左边小标题，右边可选的说明；返回这一行，按键页往右边再放分段控件。 */
    private static LinearLayout header(Context context, LinearLayout parent, String title,
            @Nullable String trailing) {
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(row);
        ViewPolicy.setHorizontalPaddingDp(context, row, 4);
        TextView heading = ViewPolicy.headingLabel(context, title, 13, ThemeColorPolicy.subText(context));
        row.addView(heading, LayoutPolicy.weightedWrapParams(1f));
        if (trailing != null) row.addView(ViewPolicy.label(context, trailing, 13, ThemeColorPolicy.subText(context)));
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthWrapParams();
        params.topMargin = DimensionPolicy.pixels(context, 22);
        // 最小 32 dp 而不是固定 32 dp：系统字体调大后标题和右侧的分段控件都比它高。
        ViewPolicy.setMinimumHeight(row, DimensionPolicy.pixels(context, 32));
        parent.addView(row, params);
        return row;
    }

    /** 并排两张 KPI 卡。 */
    private static void tiles(Context context, LinearLayout parent, View left, View right) {
        LinearLayout row = LayoutPolicy.row(context);
        LinearLayout.LayoutParams leftParams = LayoutPolicy.weightedMatchParentParams(1f);
        LinearLayout.LayoutParams rightParams = LayoutPolicy.weightedMatchParentParams(1f);
        rightParams.setMarginStart(DimensionPolicy.pixels(context, 10));
        row.addView(left, leftParams);
        row.addView(right, rightParams);
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthWrapParams();
        params.topMargin = DimensionPolicy.pixels(context, 10);
        parent.addView(row, params);
    }

    /** 一张 KPI 卡：标题、大数字和单位、一行说明；`highlight` 时说明用 accent（环比）。 */
    private static View tile(Context context, String title, String value, String unit, String note,
            boolean highlight) {
        LinearLayout tile = LayoutPolicy.column(context);
        int pad = DimensionPolicy.pixels(context, 14);
        ViewPolicy.setSymmetricPadding(tile, pad);
        ViewPolicy.setBackground(tile, DrawablePolicy.rounded(ThemeColorPolicy.card(context), DimensionPolicy.pixels(context, 20)));
        tile.addView(ViewPolicy.label(context, title, 13, ThemeColorPolicy.text(context)));
        TextView number = ViewPolicy.label(context, figure(context, value, 24,
            "—".equals(value) ? "" : unit), 24, ThemeColorPolicy.text(context));
        ViewPolicy.setPaddingDp(context, number, 0, 6, 0, 6);
        tile.addView(number);
        tile.addView(ViewPolicy.label(context, note, 12, highlight ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.subText(context)));
        tile.setContentDescription(title + " " + value + ("—".equals(value) ? "" : " " + unit) + "，" + note);
        ViewPolicy.setImportantForAccessibility(tile, View.IMPORTANT_FOR_ACCESSIBILITY_YES);
        return tile;
    }

    /** 大数字加小号单位：`12,846 字`。 */
    private static CharSequence figure(Context context, String value, int sizeSp, String unit) {
        SpannableStringBuilder text = new SpannableStringBuilder(value);
        text.setSpan(new AbsoluteSizeSpan(sizeSp, true), 0, value.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        text.setSpan(new android.text.style.StyleSpan(Typeface.BOLD), 0, value.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        text.setSpan(new ForegroundColorSpan(ThemeColorPolicy.text(context)), 0, value.length(),
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        if (!unit.isEmpty()) {
            int start = text.length();
            text.append(' ').append(unit);
            text.setSpan(new AbsoluteSizeSpan(
                BoundsPolicy.bounded(sizeSp / 3, 12, Integer.MAX_VALUE), true), start, text.length(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
            text.setSpan(new ForegroundColorSpan(ThemeColorPolicy.text(context)), start, text.length(),
                Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        }
        return text;
    }

}
