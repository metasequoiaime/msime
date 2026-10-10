package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.CloudApi;
import app.msime.android.CloudClipboardApi;
import app.msime.android.R;
import app.msime.android.ViewPolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.ZoneId;
import java.time.format.DateTimeParseException;
import java.util.List;
import java.util.concurrent.Callable;

/**
 * 云剪贴板：主开关与保留时长一张卡，下面是「最近」列表（文本、置顶 · 设备 · 时间，行尾置顶与删除），右上角「清空」。
 *
 * <p>文案如实写：记录在登录的设备之间同步，经 HTTPS 传输，明文保存在水杉云，关闭即删除。客户端不加密，所以不写「端到端加密」。保留时长 1 / 7 / 30 天或一直保留，由服务端定期清理过期记录（{@link CloudClipboardApi}）。这一页从不读取系统剪贴板，点一条记录才把它复制过去。
 *
 * <p>只有真实账号有云剪贴板；没有登录时整页只显示一句说明。
 */
public final class CloudClipboardPage extends DetailPage {
    private static final String DESCRIPTION = "在你登录的设备之间同步，经 HTTPS 传输，保存在水杉云，关闭即删除";
    private static final List<String> RETENTION_LABELS = List.of("1 天", "7 天", "30 天", "一直");

    @Nullable private CloudClipboardApi.Page page;
    private boolean busy;
    private boolean signedOut;
    @Nullable private LinearLayout column;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        render();
        if (page == null && !busy) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        super.onDestroyView();
    }

    @Override protected void onBecameVisible() {
        if (getView() != null && !busy) reload();
    }

    private CloudClipboardApi api() {
        return new CloudClipboardApi(new CloudApi(requireContext().getApplicationContext()));
    }

    private void reload() {
        CloudClipboardApi api = api();
        busy = true;
        AboutPage.network(this, api::load, outcome -> {
            busy = false;
            if (outcome.error() instanceof CloudApi.Failure failure && failure.signedOut()) {
                signedOut = true;
                page = null;
            } else if (outcome.value() != null) {
                signedOut = false;
                page = outcome.value();
            } else {
                MsToast.show(requireContext(), "云剪贴板没有读出来，请稍后再试");
            }
            render();
        });
    }

    /** 先做一次写入，成功后重新读一遍；失败只提示，界面保持上一次读到的状态。 */
    private void mutate(Callable<Void> work, @Nullable String done) {
        if (busy) {
            // 上一次读写还没回来：这次不发，但开关和分段已经被拨过去了，重画一遍让界面回到真实状态，并说一声。
            MsToast.show(requireContext(), "正在同步，稍后再试");
            render();
            return;
        }
        busy = true;
        AboutPage.network(this, work, outcome -> {
            busy = false;
            if (outcome.error() != null) {
                MsToast.show(requireContext(), outcome.error() instanceof CloudApi.Failure failure && failure.network()
                    ? "连不上服务器，请检查网络后重试" : "没有完成，请稍后再试");
                render();
                return;
            }
            if (done != null) MsToast.show(requireContext(), done);
            reload();
        });
    }

    private void render() {
        LinearLayout target = column;
        if (target == null) return;
        Context context = target.getContext();
        target.removeAllViews();

        if (signedOut) {
            GroupCard card = GroupCard.add(target, null);
            card.note("登录水杉账号后，复制的文字可以在你的设备之间同步。" + DESCRIPTION + "。");
            return;
        }

        CloudClipboardApi.Page current = page;
        GroupCard settings = GroupCard.add(target, null).withDividers(Ui.ROW_PADDING_H);
        GroupCard.Row toggle = settings.toggle("云剪贴板", DESCRIPTION, current != null && current.enabled(), this::setEnabled);
        toggle.setEnabled(current != null);

        LinearLayout retention = Ui.row(context);
        ViewPolicy.setCenteredVertically(retention);
        Ui.setRowMinimumHeight(retention, context);
        Ui.setRowPadding(retention, context);
        TextView label = Ui.styledLabel(context, "保留时长", Ui.TEXT_ROW_TITLE, 400, Ui.text(context));
        retention.addView(label, Ui.weightWrap(1f));
        SegmentedControl segments = new SegmentedControl(context);
        int selected = current == null ? -1 : CloudClipboardApi.RETENTION_DAYS.indexOf(current.retentionDays());
        segments.setOptions(RETENTION_LABELS, selected);
        segments.setContentDescription("保留时长");
        // 云剪贴板关着时只改外观不够，分段照样能点、照样发请求；关着就不响应。
        boolean retentionEditable = current != null && current.enabled();
        segments.setOnSelect(index -> {
            if (retentionEditable) setRetention(CloudClipboardApi.RETENTION_DAYS.get(index));
            else render();
        });
        retention.addView(segments);
        ViewPolicy.setEnabledWithAlpha(retention, current != null && current.enabled(), 0.38f);
        settings.addView(retention);

        if (current == null) return;

        LinearLayout header = Ui.row(context);
        ViewPolicy.setCenteredVertically(header);
        Ui.setPaddingDp(header, context, Ui.GROUP_TITLE_INSET, 0,
            Ui.GROUP_TITLE_INSET, 2);
        TextView recent = Ui.groupHeading(context, "最近");
        header.addView(recent, Ui.weightWrap(1f));
        if (!current.items().isEmpty()) {
            TextView clear = Ui.styledLabel(context, "清空", Ui.TEXT_GROUP_TITLE, 500, Ui.accent(context));
        Ui.setPaddingDp(clear, context, 8, 4, 0, 4);
            Ui.makeClickable(clear, context, this::confirmClear);
            header.addView(clear);
        }
        LinearLayout.LayoutParams headerParams = Ui.matchWidth();
        headerParams.topMargin = Ui.dp(context, Ui.GROUP_GAP);
        target.addView(header, headerParams);

        GroupCard list = GroupCard.add(target, null).withDividers(Ui.ROW_PADDING_H);
        ((LinearLayout.LayoutParams) list.view().getLayoutParams()).topMargin = Ui.dp(context, 2);
        if (current.items().isEmpty()) {
            LinearLayout empty = Ui.column(context);
            ViewPolicy.setCenteredHorizontally(empty);
            Ui.setSymmetricPaddingDp(empty, context, 16, 32);
            TextView title = Ui.styledLabel(context, "还没有同步内容", Ui.TEXT_ROW_TITLE, 500, Ui.text(context));
            empty.addView(title);
            TextView hint = Ui.centeredLabel(context, "在任一设备上复制文字，这里就会出现",
                Ui.TEXT_ROW_SUBTITLE, 400, Ui.subText(context));
            LinearLayout.LayoutParams hintParams = Ui.wrap();
            hintParams.topMargin = Ui.dp(context, 4);
            empty.addView(hint, hintParams);
            list.addView(empty);
            return;
        }
        for (CloudClipboardApi.Item item : current.items()) list.addView(itemRow(context, item));
    }

    private View itemRow(Context context, CloudClipboardApi.Item item) {
        LinearLayout row = Ui.row(context);
        ViewPolicy.setCenteredVertically(row);
        Ui.setPaddingDp(row, context, 16, 12, 8, 12);

        LinearLayout texts = Ui.column(context);
        TextView text = Ui.styledLabel(context, item.text(), 15, 400, Ui.text(context));
        ViewPolicy.setMaxLinesEllipsized(text, 3);
        texts.addView(text);
        TextView meta = Ui.styledLabel(context, meta(item), 12, 400, Ui.subText(context));
        LinearLayout.LayoutParams metaParams = Ui.wrap();
        metaParams.topMargin = Ui.dp(context, 4);
        texts.addView(meta, metaParams);
        row.addView(texts, Ui.weightWrap(1f));

        row.addView(Ui.iconButton(context, R.drawable.ic_ms_keep,
            item.pinned() ? Ui.accent(context) : Ui.subText(context),
            item.pinned() ? "取消置顶" : "置顶", 40, () -> {
                CloudClipboardApi api = api();
                mutate(() -> { api.setPinned(item.id(), !item.pinned()); return null; }, null);
            }));
        row.addView(Ui.iconButton(context, R.drawable.ic_ms_delete, Ui.subText(context), "删除", 40, () -> {
            CloudClipboardApi api = api();
            mutate(() -> { api.delete(item.id()); return null; }, "已删除");
        }));

        row.setContentDescription(item.text() + "，" + meta(item) + "，点按复制");
        Ui.makeClickable(row, context, () -> {
            ClipboardActions.copyText(context, "水杉云剪贴板", item.text(), "已复制");
        });
        return row;
    }

    /** 「已置顶 · 设备 · 时间」，没有的部分省掉。 */
    private static String meta(CloudClipboardApi.Item item) {
        String when = relative(item.updatedAt());
        int capacity = (item.pinned() ? 3 : 0) + item.device().length() + when.length() + 6;
        StringBuilder out = new StringBuilder(capacity);
        if (item.pinned()) out.append("已置顶");
        if (!item.device().isEmpty()) {
            if (out.length() > 0) out.append(" · ");
            out.append(item.device());
        }
        if (!when.isEmpty()) {
            if (out.length() > 0) out.append(" · ");
            out.append(when);
        }
        return out.toString();
    }

    /** 刚刚、N 分钟前、N 小时前、昨天，再早就写月日。读不出时间时为空。 */
    static String relative(String timestamp) {
        Instant then;
        try {
            then = Instant.parse(timestamp);
        } catch (DateTimeParseException malformed) {
            return "";
        }
        Instant now = Instant.now();
        long minutes = Duration.between(then, now).toMinutes();
        if (minutes < 1) return "刚刚";
        if (minutes < 60) return minutes + " 分钟前";
        ZoneId zone = ZoneId.systemDefault();
        LocalDate day = then.atZone(zone).toLocalDate();
        LocalDate today = now.atZone(zone).toLocalDate();
        if (day.equals(today)) return (minutes / 60) + " 小时前";
        if (day.equals(today.minusDays(1))) return "昨天";
        if (day.getYear() == today.getYear()) return day.getMonthValue() + " 月 " + day.getDayOfMonth() + " 日";
        return day.getYear() + " 年 " + day.getMonthValue() + " 月 " + day.getDayOfMonth() + " 日";
    }

    private void setEnabled(boolean enabled) {
        CloudClipboardApi api = api();
        if (enabled) {
            mutate(() -> { api.setEnabled(true); return null; }, null);
            return;
        }
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle("关闭云剪贴板？")
            .setMessage("关闭后，保存在水杉云上的全部记录都会删除，其他设备也看不到了。")
            .setNegativeButton("取消", (dialog, which) -> render())
            .setOnCancelListener(dialog -> render())
            .setPositiveButton("关闭", (dialog, which) -> mutate(() -> { api.setEnabled(false); return null; }, "已关闭"))
            .show();
    }

    private void setRetention(int days) {
        CloudClipboardApi.Page current = page;
        if (current == null || current.retentionDays() == days) return;
        CloudClipboardApi api = api();
        mutate(() -> { api.setRetention(days); return null; }, null);
    }

    private void confirmClear() {
        CloudClipboardApi api = api();
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle("清空云剪贴板？")
            .setMessage("所有设备上同步的记录都会删除，置顶的也不例外。")
            .setNegativeButton("取消", null)
            .setPositiveButton("清空", (dialog, which) -> mutate(() -> { api.clear(); return null; }, "已清空"))
            .show();
    }
}
