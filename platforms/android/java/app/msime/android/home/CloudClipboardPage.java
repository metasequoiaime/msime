package app.msime.android.home;

import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import app.msime.android.ImageViewPolicy;
import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.graphics.Typeface;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.CloudApi;
import app.msime.android.CloudClipboardApi;
import app.msime.android.CloudClipboardNativeDriver;
import app.msime.android.R;
import app.msime.android.ViewPolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.util.List;
import java.util.concurrent.Callable;
import app.msime.android.CloudClipboardTimePolicy;

/**
 * 云剪贴板：主开关与保留时长一张卡；本机等待允许时是本机的核对码和说明；本机能读时依次是「等待允许」的其他设备（允许 / 拒绝）、「最近」列表（文本、置顶 · 设备 · 时间，行尾置顶与删除，右上角「清空」）和「设备」列表（移除、重置）。
 *
 * <p>文案如实写：端到端加密，文字在设备上加密后才上传，水杉云只保存密文，关闭即删除（`docs/cloud-clipboard-e2e.md`）。不用口令：新设备由已有设备允许，允许前两台设备上显示同一串 16 位核对码，让用户核对一致再点。移除设备和重置都会换一把只有本机持有的新密钥、删掉云端全部记录，其他设备都要在本机上重新允许，确认框里写明这一点。保留时长 1 / 7 / 30 天或一直保留，由服务端定期清理过期记录（{@link CloudClipboardApi}）。这一页从不读取系统剪贴板，点一条记录才把它复制过去。
 *
 * <p>只有真实账号有云剪贴板；没有登录时整页只显示一句说明。
 */
public final class CloudClipboardPage extends ReloadingDetailPage {
    private static final String DESCRIPTION = "在你登录的设备之间同步，端到端加密，水杉云只保存密文，关闭即删除";
    private static final String KEY_CHANGE = "云端现有的记录会全部删除，其他设备都要在这台设备上重新允许后才能继续同步。";
    private static final List<String> RETENTION_LABELS = List.of("1 天", "7 天", "30 天", "一直");
    private record Loaded(CloudClipboardApi api, CloudClipboardApi.Overview overview) {}

    @Nullable private CloudClipboardApi.Overview overview;
    @Nullable private CloudClipboardApi loadedApi;
    private boolean busy;
    private boolean signedOut;
    private long generation;
    @Override protected void onContentReady() {
        render();
    }

    @Override protected void onContentDestroyed() {
        generation++;
        overview = null;
        loadedApi = null;
        busy = false;
        signedOut = false;
    }

    @Override protected void reload() {
        Context application = requireContext().getApplicationContext();
        long request = ++generation;
        busy = true;
        overview = null;
        loadedApi = null;
        signedOut = false;
        render();
        NetworkTask.run(this, () -> {
            CloudApi cloud = new CloudApi(application);
            CloudClipboardApi api = new CloudClipboardApi(
                cloud.forAccountSession(cloud.currentAccountSessionId()),
                CloudClipboardNativeDriver.local(application));
            return new Loaded(api, api.load());
        }, outcome -> {
            if (request != generation) return;
            busy = false;
            if (outcome.error() instanceof CloudApi.Failure failure && failure.signedOut()) {
                signedOut = true;
            } else if (outcome.value() != null) {
                signedOut = false;
                loadedApi = outcome.value().api();
                overview = outcome.value().overview();
            } else {
                MsToast.show(requireContext(), outcome.error() instanceof CloudApi.Failure failure
                    ? failureMessage(failure, "云剪贴板没有读出来，请稍后再试") : "云剪贴板没有读出来，请稍后再试");
            }
            render();
        });
    }

    /** Write through the login that supplied this page, then reload; a failed write discards the old rows. */
    private void mutate(Callable<?> work, @Nullable String done) {
        if (busy) {
            // 上一次读写还没回来：这次不发，但开关和分段已经被拨过去了，重画一遍让界面回到真实状态，并说一声。
            MsToast.show(requireContext(), "正在同步，稍后再试");
            render();
            return;
        }
        busy = true;
        long request = ++generation;
        NetworkTask.run(this, work, outcome -> {
            if (request != generation) return;
            busy = false;
            if (outcome.error() != null) {
                MsToast.show(requireContext(), outcome.error() instanceof CloudApi.Failure failure
                    ? failureMessage(failure, "没有完成，请稍后再试") : "没有完成，请稍后再试");
                overview = null;
                loadedApi = null;
                render();
                return;
            }
            if (done != null) MsToast.show(requireContext(), done);
            reload();
        });
    }

    /** 失败时给人看的一句话；驱动的错误码在 {@link CloudApi.Failure#code} 里。 */
    static String failureMessage(CloudApi.Failure failure, String fallback) {
        if (failure.network()) return "连不上服务器，请检查网络后重试";
        return switch (failure.code) {
            case "clipboard_key_conflict" -> "另一台设备刚刚改了云剪贴板，请稍后再试";
            case "clipboard_device_limit" -> "这个账号登记的设备已满 20 台，请先移除不用的设备";
            case "clipboard_sync_disabled" -> "云剪贴板未开启";
            case "clipboard_device_pending" -> "这台设备还在等待允许";
            case "not_found" -> "那台设备已经不在了，请刷新后再试";
            case "session_changed" -> "登录已改变，请重新打开页面";
            default -> fallback;
        };
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        Context context = target.getContext();
        target.removeAllViews();

        if (signedOut) {
            GroupCard card = GroupCard.add(target, null);
            card.note("登录水杉账号后，复制的文字可以在你的设备之间同步。" + DESCRIPTION + "。");
            return;
        }

        CloudClipboardApi.Overview current = overview;
        CloudClipboardApi.Page page = current == null ? null : current.page();
        GroupCard settings = GroupCard.add(target, null).withDividers(Ui.ROW_PADDING_H);
        GroupCard.Row toggle = settings.toggle("云剪贴板", DESCRIPTION, page != null && page.enabled(), this::setEnabled);
        toggle.setEnabled(page != null);

        LinearLayout retention = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(retention);
        ViewPolicy.setRowMinimumHeight(context, retention);
        ViewPolicy.setRowPadding(context, retention);
        TextView label = ViewPolicy.styledLabel(context, "保留时长", Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
        retention.addView(label, LayoutPolicy.weightedWrapParams(1f));
        SegmentedControl segments = new SegmentedControl(context);
        int selected = page == null ? -1 : CloudClipboardApi.RETENTION_DAYS.indexOf(page.retentionDays());
        segments.setOptions(RETENTION_LABELS, selected);
        segments.setContentDescription("保留时长");
        // 云剪贴板关着时只改外观不够，分段照样能点、照样发请求；关着就不响应。
        boolean retentionEditable = page != null && page.enabled();
        segments.setOnSelect(index -> {
            if (retentionEditable) setRetention(CloudClipboardApi.RETENTION_DAYS.get(index));
            else render();
        });
        retention.addView(segments);
        ViewPolicy.setEnabledWithAlpha(retention, retentionEditable, 0.38f);
        settings.addView(retention);

        if (current == null || page == null || !page.enabled()) return;
        CloudClipboardApi.Devices devices = current.devices();
        if (!CloudClipboardApi.STATUS_READY.equals(devices.status())) {
            renderWaiting(target, context, devices);
            return;
        }
        renderPendingDevices(target, context, devices.pending());
        renderItems(target, context, page);
        renderDevices(target, devices);
    }

    /** 本机还没被允许：本机的核对码、去哪里允许，以及没有其他设备可用时的「重置」。 */
    private void renderWaiting(LinearLayout target, Context context, CloudClipboardApi.Devices devices) {
        GroupCard card = GroupCard.add(target, "这台设备等待允许").withDividers(Ui.ROW_PADDING_H);
        card.note("在另一台已经在用云剪贴板的设备上打开「我的 → 云剪贴板」，那里会列出这台设备。核对那里显示的核对码和下面这串一致后，点「允许」。");
        LinearLayout code = LayoutPolicy.column(context);
        ViewPolicy.setSymmetricPaddingDp(context, code, Ui.ROW_PADDING_H, 14);
        TextView caption = ViewPolicy.styledLabel(context, "本机核对码", 13, 400, ThemeColorPolicy.subText(context));
        code.addView(caption);
        TextView digits = codeLabel(context, devices.code(), 22);
        LinearLayout.LayoutParams digitsParams = LayoutPolicy.wrapParams();
        digitsParams.topMargin = DimensionPolicy.pixels(context, 4);
        code.addView(digits, digitsParams);
        card.addView(code);
        card.button("重置云剪贴板", "手边没有别的设备时用", "重置", this::confirmReset);
        card.footer("允许之后这里会显示同步的内容。");
    }

    /** 等本机允许的其他设备：名称、登记时间、核对码，行尾「拒绝」「允许」。 */
    private void renderPendingDevices(LinearLayout target, Context context, List<CloudClipboardApi.Device> pending) {
        if (pending.isEmpty()) return;
        GroupCard card = GroupCard.add(target, "等待允许").withDividers(Ui.ROW_PADDING_H);
        for (CloudClipboardApi.Device device : pending) {
            LinearLayout row = LayoutPolicy.row(context);
            ViewPolicy.setCenteredVertically(row);
            ViewPolicy.setPaddingDp(context, row, 16, 12, 12, 12);
            LinearLayout texts = LayoutPolicy.column(context);
            texts.addView(ViewPolicy.styledLabel(context, deviceName(device), 15, 500, ThemeColorPolicy.text(context)));
            String when = CloudClipboardTimePolicy.relative(device.createdAt());
            if (!when.isEmpty()) {
                TextView meta = ViewPolicy.styledLabel(context, when + "申请", 12, 400, ThemeColorPolicy.subText(context));
                LinearLayout.LayoutParams metaParams = LayoutPolicy.wrapParams();
                metaParams.topMargin = DimensionPolicy.pixels(context, 2);
                texts.addView(meta, metaParams);
            }
            LinearLayout.LayoutParams codeParams = LayoutPolicy.wrapParams();
            codeParams.topMargin = DimensionPolicy.pixels(context, 4);
            texts.addView(codeLabel(context, device.code(), 15), codeParams);
            row.addView(texts, LayoutPolicy.weightedWrapParams(1f));
            row.addView(KeyboardSheets.tonalButton(context, "拒绝", "拒绝" + deviceName(device), 500,
                () -> reject(device)), LayoutPolicy.rowGapParams(context, 8));
            row.addView(KeyboardSheets.tonalButton(context, "允许", "允许" + deviceName(device), 500,
                () -> confirmApprove(device)), LayoutPolicy.rowGapParams(context, 8));
            card.addView(row);
        }
        card.footer("核对两台设备上的核对码一致再允许。不认识的设备点「拒绝」。");
    }

    private void renderItems(LinearLayout target, Context context, CloudClipboardApi.Page page) {
        LinearLayout header = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(header);
        ViewPolicy.setPaddingDp(context, header, Ui.GROUP_TITLE_INSET, 0,
            Ui.GROUP_TITLE_INSET, 2);
        TextView recent = ViewPolicy.headingLabel(context, "最近", Ui.TEXT_GROUP_TITLE, 500,
            ThemeColorPolicy.accent(context));
        header.addView(recent, LayoutPolicy.weightedWrapParams(1f));
        if (!page.items().isEmpty()) {
            TextView clear = ViewPolicy.styledLabel(context, "清空", Ui.TEXT_GROUP_TITLE, 500, ThemeColorPolicy.accent(context));
            ViewPolicy.setPaddingDp(context, clear, 8, 4, 0, 4);
            ViewPolicy.makeClickable(clear, context, this::confirmClear);
            header.addView(clear);
        }
        LinearLayout.LayoutParams headerParams = LayoutPolicy.matchWidthWrapParams();
        headerParams.topMargin = DimensionPolicy.pixels(context, Ui.GROUP_GAP);
        target.addView(header, headerParams);

        GroupCard list = GroupCard.add(target, null).withDividers(Ui.ROW_PADDING_H);
        ((LinearLayout.LayoutParams) list.view().getLayoutParams()).topMargin = DimensionPolicy.pixels(context, 2);
        int undecryptable = page.e2e().undecryptable();
        if (undecryptable > 0) {
            list.footer("另有 " + undecryptable + " 条在这台设备上解不开，没有列出：多半是本机被允许之前、用换掉的密钥写入的。");
        }
        if (page.items().isEmpty()) {
            LinearLayout empty = LayoutPolicy.column(context);
            ViewPolicy.setCenteredHorizontally(empty);
            ViewPolicy.setSymmetricPaddingDp(context, empty, 16, 32);
            TextView title = ViewPolicy.styledLabel(context, "还没有同步内容", Ui.TEXT_ROW_TITLE, 500, ThemeColorPolicy.text(context));
            empty.addView(title);
            TextView hint = ViewPolicy.centeredLabel(context, "在任一设备上复制文字，这里就会出现",
                Ui.TEXT_ROW_SUBTITLE, 400, ThemeColorPolicy.subText(context));
            LinearLayout.LayoutParams hintParams = LayoutPolicy.wrapParams();
            hintParams.topMargin = DimensionPolicy.pixels(context, 4);
            empty.addView(hint, hintParams);
            list.addView(empty);
            return;
        }
        for (CloudClipboardApi.Item item : page.items()) list.addView(itemRow(context, item));
    }

    /** 已允许的设备：本机标出「本机」，其他设备行尾「移除」；最后一行是「重置」。 */
    private void renderDevices(LinearLayout target, CloudClipboardApi.Devices devices) {
        GroupCard card = GroupCard.add(target, "设备").withDividers(Ui.ROW_PADDING_H);
        for (CloudClipboardApi.Device device : devices.approved()) {
            String when = CloudClipboardTimePolicy.relative(device.createdAt());
            String subtitle = "核对码 " + device.code() + (when.isEmpty() ? "" : " · " + when + "加入");
            if (device.current()) {
                card.value(deviceName(device), subtitle, "本机");
            } else {
                card.button(deviceName(device), subtitle, "移除", () -> confirmRemove(device));
            }
        }
        card.button("重置云剪贴板", "换一把新密钥，所有设备重新允许", "重置", this::confirmReset);
    }

    private static String deviceName(CloudClipboardApi.Device device) {
        return device.name().isEmpty() ? "未知设备" : device.name();
    }

    /** 核对码用等宽字显示，读屏逐位念。 */
    private static TextView codeLabel(Context context, String code, int sizeSp) {
        TextView digits = ViewPolicy.styledLabel(context, code, sizeSp, 500, ThemeColorPolicy.text(context));
        ViewPolicy.setTypeface(digits, Typeface.MONOSPACE, Typeface.BOLD);
        StringBuilder spoken = new StringBuilder("核对码");
        for (int index = 0; index < code.length(); index++) {
            char c = code.charAt(index);
            if (c != ' ') spoken.append(' ').append(c);
        }
        digits.setContentDescription(spoken);
        return digits;
    }

    private View itemRow(Context context, CloudClipboardApi.Item item) {
        LinearLayout row = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(row);
        ViewPolicy.setPaddingDp(context, row, 16, 12, 8, 12);

        LinearLayout texts = LayoutPolicy.column(context);
        TextView text = ViewPolicy.styledLabel(context, item.text(), 15, 400, ThemeColorPolicy.text(context));
        ViewPolicy.setMaxLinesEllipsized(text, 3);
        texts.addView(text);
        TextView meta = ViewPolicy.styledLabel(context, meta(item), 12, 400, ThemeColorPolicy.subText(context));
        LinearLayout.LayoutParams metaParams = LayoutPolicy.wrapParams();
        metaParams.topMargin = DimensionPolicy.pixels(context, 4);
        texts.addView(meta, metaParams);
        row.addView(texts, LayoutPolicy.weightedWrapParams(1f));

        row.addView(ImageViewPolicy.iconButton(context, R.drawable.ic_ms_keep,
            item.pinned() ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.subText(context),
            item.pinned() ? "取消置顶" : "置顶", 40, () -> {
                CloudClipboardApi api = loadedApi;
                if (api == null) return;
                mutate(() -> { api.setPinned(item.id(), !item.pinned()); return null; }, null);
            }));
        row.addView(ImageViewPolicy.iconButton(context, R.drawable.ic_ms_delete, ThemeColorPolicy.subText(context), "删除", 40, () -> {
            CloudClipboardApi api = loadedApi;
            if (api == null) return;
            mutate(() -> { api.delete(item.id()); return null; }, "已删除");
        }));

        row.setContentDescription(item.text() + "，" + meta(item) + "，点按复制");
        ViewPolicy.makeClickable(row, context, () -> copy(item));
        return row;
    }

    private void copy(CloudClipboardApi.Item item) {
        CloudClipboardApi source = loadedApi;
        if (source == null) return;
        long request = generation;
        NetworkTask.run(this, () -> {
            source.requireCurrentSession();
            return item.text();
        }, outcome -> {
            if (request != generation || loadedApi != source) return;
            if (outcome.error() != null) {
                reload();
                return;
            }
            ClipboardActions.copyText(requireContext(), "水杉云剪贴板", outcome.value(), "已复制");
        });
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
        return CloudClipboardTimePolicy.relative(timestamp);
    }

    private void setEnabled(boolean enabled) {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        if (enabled) {
            mutate(() -> { api.setEnabled(true); return null; }, null);
            return;
        }
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle("关闭云剪贴板？")
            .setMessage("关闭后，保存在水杉云上的全部记录都会删除，所有设备的登记也一起删除，再次开启时其他设备要重新允许。")
            .setNegativeButton("取消", (dialog, which) -> render())
            .setOnCancelListener(dialog -> render())
            .setPositiveButton("关闭", (dialog, which) -> mutate(() -> { api.setEnabled(false); return null; }, "已关闭"))
            .show();
    }

    private void setRetention(int days) {
        CloudClipboardApi.Overview current = overview;
        if (current == null || current.page().retentionDays() == days) return;
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        mutate(() -> { api.setRetention(days); return null; }, null);
    }

    private void confirmClear() {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle("清空云剪贴板？")
            .setMessage("所有设备上同步的记录都会删除，置顶的也不例外。")
            .setNegativeButton("取消", null)
            .setPositiveButton("清空", (dialog, which) -> mutate(() -> { api.clear(); return null; }, "已清空"))
            .show();
    }

    private void confirmApprove(CloudClipboardApi.Device device) {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle("允许「" + deviceName(device) + "」？")
            .setMessage("核对两台设备上的核对码一致再允许：\n\n" + device.code()
                + "\n\n允许后，那台设备可以读取和上传这个账号的云剪贴板。")
            .setNegativeButton("取消", null)
            .setPositiveButton("允许", (dialog, which) -> mutate(() -> api.approve(device.deviceId()), "已允许"))
            .show();
    }

    private void reject(CloudClipboardApi.Device device) {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        mutate(() -> api.reject(device.deviceId()), "已拒绝");
    }

    private void confirmRemove(CloudClipboardApi.Device device) {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle("移除「" + deviceName(device) + "」？")
            .setMessage("移除会换一把新的密钥，只交给这台设备。" + KEY_CHANGE)
            .setNegativeButton("取消", null)
            .setPositiveButton("移除", (dialog, which) -> mutate(() -> api.remove(device.deviceId()), "已移除"))
            .show();
    }

    private void confirmReset() {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle("重置云剪贴板？")
            .setMessage("重置会换一把新的密钥，只交给这台设备。" + KEY_CHANGE)
            .setNegativeButton("取消", null)
            .setPositiveButton("重置", (dialog, which) -> mutate(() -> api.reset(), "已重置"))
            .show();
    }
}
