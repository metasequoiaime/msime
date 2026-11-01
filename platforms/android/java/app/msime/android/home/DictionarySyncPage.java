package app.msime.android.home;

import android.content.Context;
import android.graphics.Typeface;
import android.text.InputType;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.CloudClipboardApi;
import app.msime.android.CloudClipboardTimePolicy;
import app.msime.android.DeviceNamePolicy;
import app.msime.android.DictionaryWebDavSync;
import app.msime.android.DimensionPolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.NumberPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.clipboard.CloudClipboardRelayPolicy;
import app.msime.android.policy.DictionarySyncPolicy;
import java.time.DateTimeException;
import java.time.Instant;
import java.time.ZoneId;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 词库同步：「我的词条」经用户自己的 WebDAV 网盘在设备之间同步（`docs/dictionary-webdav-sync.md`）。同步的是「我的词条」里的词和它们的删除，权重、候选顺序和主词库都不同步；网盘只保存密文，新设备要在已有设备上核对后允许，与云剪贴板分开允许。
 *
 * <p>三组：「设置」是网盘的服务器地址、账号和应用密码，在对话框里填，规则与云剪贴板的 WebDAV 相同（{@link CloudClipboardRelayPolicy}）；应用密码不回显，已经保存过时留空表示保留原来的；「测试连接」只测不存，「保存」先测，通过了才写进偏好 `dictionary_sync_webdav`，没通过时对话框和填好的内容都留着，原因写在输入框下面。「状态」是上次同步的时间、共享词库的词数、待发送和待写入的改动、被挡下的大批删除（「同步删除」或「从网盘恢复」）和「立即同步」。「设备」是本机等待允许时的核对码，等本机允许的其他设备（允许 / 拒绝），已允许的设备（移除），以及很少用到的「重置密钥」和「以本机为准重新开始」，都要确认。
 *
 * <p>打开这一页时以 `opened` 同步一次（本机第一次用这个网盘时由这里登记，键盘进程从不登记），「立即同步」是 `manual`；都在工作线程上跑（{@link DictionaryWebDavSync}），要不要联网、额度和退避由 Rust 决定。错误码的说法在 {@link DictionarySyncPolicy#failureMessage}，与共享设置页一致。
 */
public final class DictionarySyncPage extends ReloadingDetailPage {
    @Override protected List<String> sectionTitles() {
        return List.of("设置", "状态", "设备");
    }

    /** 网盘的说明，与共享设置页同一段的意思相同。 */
    private static final String NOTE = "词在本机加密，网盘只保存密文；打开这一页和改词之后自动同步，没有实时推送；坚果云免费账号每 30 分钟 600 次请求与其他应用共用，词库同步每台设备自动最多用 40 次；建议使用专用应用密码，移除设备后更换应用密码；已删除的密文可能留在网盘回收站。";
    private static final String DESCRIPTION = "「我的词条」里的词和它们的删除在你的设备之间经 WebDAV 网盘同步，端到端加密；打字学到的权重、候选顺序和主词库不同步。";
    private static final String KEY_CHANGE = "其他设备都要在这台设备上重新允许后才能继续同步，各设备上的词都不会删除。";

    /** 偏好里的网盘：`password` 只记有没有，不留在页面上。 */
    private record Config(String url, String username, boolean passwordSaved) {
        boolean configured() { return !url.isEmpty(); }
    }
    /** 一次读取或操作之后的全部：同步状态（读不到时为 null）、这一次的错误码、设备列表（读不到时为 null）和读设备时的错误码。 */
    private record Loaded(@Nullable DictionaryWebDavSync.Status status, @Nullable String error,
            @Nullable CloudClipboardApi.Devices devices, @Nullable String devicesError) {}
    /** 一次「测试连接」的结果：`code` 为 null 表示通过；`password` 是实际测试用的应用密码（留空保持不变时是已存的那个），通过后「保存」原样写进偏好。 */
    private record Probe(@Nullable String code, String password) {}
    /** 仅在恢复确认期间持有的已存连接，不显示或记录应用密码。 */
    private record SavedConnection(String url, String username, String password) {}
    /** 用户确认期间已存连接发生变化，终止整项任务，不继续读取新网盘的状态或设备。 */
    private static final class SavedConnectionChanged extends RuntimeException {
        private static final long serialVersionUID = 1L;
    }

    /** 读到的网盘配置；还没读到或偏好读不了时为 null。 */
    @Nullable private Config config;
    @Nullable private Loaded loaded;
    private boolean busy;
    // 主线程更新，网络线程在每次宿主调用前读取，不能只过滤回到界面的结果。
    private volatile long generation;

    @Override protected void onContentReady() {
        render();
    }

    @Override protected boolean shouldReloadWhenVisible() {
        return !busy;
    }

    @Override protected void onContentDestroyed() {
        generation++;
        config = null;
        loaded = null;
        busy = false;
    }

    @Override protected void reload() {
        long request = ++generation;
        busy = true;
        loaded = null;
        render();
        HostTask.run(this, DictionarySyncPage::readConfig, read -> {
            if (request != generation) return;
            config = read;
            if (read == null || !read.configured()) {
                busy = false;
                render();
                return;
            }
            render();
            work(request, sync -> sync.run(DictionarySyncPolicy.TRIGGER_OPENED, false, false), null);
        });
    }

    /** 一次同步或设备操作。 */
    private interface Action {
        /** 跑完返回这一次的同步结果（没有同步时为 null），之后再读一次设备。 */
        @Nullable DictionaryWebDavSync.Run apply(DictionaryWebDavSync sync) throws DictionaryWebDavSync.Failure;
    }

    /** 在工作线程上跑 `action`，再读一次状态和设备，回到主线程后画出来；`done` 是成功时的提示。 */
    private void work(long request, Action action, @Nullable String done) {
        Context application = requireContext().getApplicationContext();
        busy = true;
        NetworkTask.run(this, (Callable<Loaded>) () -> {
            if (request != generation) return null;
            DictionaryWebDavSync sync = DictionaryWebDavSync.open(application);
            String error = null;
            DictionaryWebDavSync.Status status = null;
            try {
                if (request != generation) return null;
                DictionaryWebDavSync.Run run = action.apply(sync);
                if (run != null) {
                    status = run.status();
                    error = run.error();
                }
            } catch (DictionaryWebDavSync.Failure failure) {
                error = failure.code;
            }
            if (status == null) {
                if (request != generation) return null;
                try {
                    status = sync.status();
                } catch (DictionaryWebDavSync.Failure failure) {
                    if (error == null) error = failure.code;
                }
            }
            CloudClipboardApi.Devices devices = null;
            String devicesError = null;
            if (DictionarySyncPolicy.unreachable(error)) {
                // 刚才连网盘就不行：不再读设备，原因已经写在状态里。
                devicesError = error;
            } else {
                // devices 会登记本机；旧任务不能在用户换过网盘后继续对新配置执行。
                if (request != generation) return null;
                try {
                    devices = sync.devices();
                } catch (DictionaryWebDavSync.Failure failure) {
                    devicesError = failure.code;
                }
            }
            return new Loaded(status, error, devices, devicesError);
        }, outcome -> {
            if (request != generation) return;
            busy = false;
            if (outcome.value() == null) {
                MsToast.show(requireContext(), outcome.error() instanceof SavedConnectionChanged
                    ? "网盘设置已更改，请重新确认恢复"
                    : outcome.error() instanceof DictionaryWebDavSync.Failure failure
                        ? DictionarySyncPolicy.failureMessage(failure.code, false) : "词库同步没有读出来，请稍后再试");
                loaded = null;
            } else {
                loaded = outcome.value();
                if (done != null && loaded.error() == null) MsToast.show(requireContext(), done);
            }
            render();
        });
    }

    /** 用户点了一个操作：上一次还没回来就不发，说一声。 */
    private void act(Action action, @Nullable String done) {
        if (busy) {
            MsToast.show(requireContext(), "正在同步，稍后再试");
            return;
        }
        long request = ++generation;
        render();
        work(request, action, done);
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        Context context = target.getContext();
        target.removeAllViews();
        Config current = config;
        renderSettings(target, current);
        if (current == null || !current.configured()) return;
        Loaded state = loaded;
        if (state == null) {
            GroupCard card = GroupCard.add(sectionColumn(target, "状态"), "同步");
            card.note(busy ? "正在同步……" : "词库同步没有读出来，请稍后再试");
            return;
        }
        renderStatus(target, state);
        renderDevices(target, context, state);
    }

    /** 「设置」：网盘的地址、账号和应用密码；没配置时只有一行「设置 WebDAV 网盘」。 */
    private void renderSettings(LinearLayout target, @Nullable Config current) {
        GroupCard card = GroupCard.add(sectionColumn(target, "设置"), "WebDAV 网盘").withDividers(Ui.ROW_PADDING_H);
        if (current == null) {
            card.note("设置还没有读出来。");
            return;
        }
        if (!current.configured()) {
            card.nav("设置 WebDAV 网盘", null, null, () -> editWebDav(current)).help("坚果云、Nextcloud 等都可以，不需要水杉账号");
            card.help(DESCRIPTION + "\n" + NOTE);
            return;
        }
        card.nav("服务器地址", null, shorten(current.url()), () -> editWebDav(current));
        card.nav("账号", null, shorten(current.username()), () -> editWebDav(current));
        card.nav("应用密码", null, current.passwordSaved() ? "已填写" : "未填写", () -> editWebDav(current));
        card.button("停止词库同步", "清除本机保存的网盘设置", "停止", this::confirmStop);
        card.footer("「我的词条」现在经 WebDAV 网盘 " + current.url() + " 同步。换了地址或账号后，本机要重新登记，并在其他设备上重新允许；换应用密码不用。");
        card.help(NOTE);
    }

    /** 「状态」：上次同步、共享词库的词数、待同步的改动、自动同步在等额度时什么时候继续、时钟提示、被挡下的删除和「立即同步」。 */
    private void renderStatus(LinearLayout target, Loaded state) {
        GroupCard card = GroupCard.add(sectionColumn(target, "状态"), "同步").withDividers(Ui.ROW_PADDING_H);
        DictionaryWebDavSync.Status status = state.status();
        card.value("状态", statusLine(state), null);
        if (status != null) {
            card.value("共享的词", null, NumberPolicy.groupedCount(status.words()));
            String pending = DictionarySyncPolicy.pendingSummary(status.pendingUpload(), status.pendingApply());
            if (!pending.isEmpty()) card.value("待同步", pending, null);
            String wait = DictionarySyncPolicy.budgetWaitMessage(status.budgetWaitUntil(), System.currentTimeMillis(),
                ZoneId.systemDefault());
            if (wait != null) card.note(wait);
        }
        card.button("立即同步", null, "同步", () -> act(
            sync -> sync.run(DictionarySyncPolicy.TRIGGER_MANUAL, false, false), "已同步")).help("不等自动同步的间隔");
        String error = state.error() != null ? state.error() : status == null ? null : status.lastError();
        if (DictionarySyncPolicy.CODE_ROLLBACK.equals(error)) {
            card.button("重新信任网盘现状", "仅在确认网盘被你清理后恢复同步", "恢复", this::confirmTrustRemote);
        }
        if (status != null && status.pendingApply() > 0) {
            card.footer("键盘正在用词库时，网盘上的改动先不写进本机，收起键盘后会写进去。");
        }
        if (status != null && status.clockSkew()) {
            card.note("另一台设备的时间比这台快一天以上，请检查各设备的系统时间。同步照常进行。");
        }
        String held = DictionarySyncPolicy.heldDeletionsMessage(status == null ? null : status.heldDeletions());
        if (held != null) {
            GroupCard deletions = GroupCard.add(sectionColumn(target, "状态"), "被挡下的删除").withDividers(Ui.ROW_PADDING_H);
            deletions.note(held);
            deletions.button("同步删除", "在所有设备上删掉这些词", "删除", () -> MessageDialog.confirm(requireContext(),
                "同步删除这 " + status.heldDeletions() + " 个词？", "其他设备上的这些词也会被删除。",
                "同步删除", () -> act(sync -> sync.run(DictionarySyncPolicy.TRIGGER_MANUAL, true, false), "已同步删除")));
            deletions.button("从网盘恢复", "把这些词写回本机", "恢复", () -> act(
                sync -> sync.run(DictionarySyncPolicy.TRIGGER_MANUAL, false, true), "已从网盘恢复"));
        }
    }

    /** 状态行：出错时是原因，否则是「已同步 · 最近 …」或还没同步过。 */
    private static String statusLine(Loaded state) {
        DictionaryWebDavSync.Status status = state.status();
        String error = state.error() != null ? state.error() : status == null ? null : status.lastError();
        if (error != null) return DictionarySyncPolicy.failureMessage(error, false);
        if (status == null) return "没有读出来";
        if (!status.approved()) return "这台设备等待允许";
        String when = status.lastSuccessMs() == null ? "" : relative(status.lastSuccessMs());
        return when.isEmpty() ? (status.joined() ? "已同步" : "还没有同步过") : "已同步 · " + when;
    }

    /** 「设备」：本机等待允许时是核对码和说明；能用时是等待允许的设备、已允许的设备和两个很少用到的操作。 */
    private void renderDevices(LinearLayout target, Context context, Loaded state) {
        CloudClipboardApi.Devices devices = state.devices();
        DictionaryWebDavSync.Status status = state.status();
        if (devices == null) {
            GroupCard card = GroupCard.add(sectionColumn(target, "设备"), "设备");
            card.note(state.devicesError() == null ? "设备没有读出来，请稍后再试"
                : DictionarySyncPolicy.failureMessage(state.devicesError(), false));
            return;
        }
        if (!CloudClipboardApi.STATUS_READY.equals(devices.status())) {
            renderWaiting(target, context, devices);
            return;
        }
        renderPending(target, context, devices.pending());
        GroupCard card = GroupCard.add(sectionColumn(target, "设备"), "设备").withDividers(Ui.ROW_PADDING_H);
        for (CloudClipboardApi.Device device : devices.approved()) {
            String when = CloudClipboardTimePolicy.relative(device.createdAt());
            String subtitle = "核对码 " + device.code() + (when.isEmpty() ? "" : " · " + when + "加入");
            if (device.current()) {
                card.value(DeviceNamePolicy.display(device.name()), subtitle, "本机");
            } else {
                card.button(DeviceNamePolicy.display(device.name()), subtitle, "移除", () -> confirmRemove(device));
            }
        }
        card.button("重置密钥", "换一把新密钥，所有设备重新允许", "重置", this::confirmReset);
        card.button("以本机为准重新开始", "持有旧密钥的设备都不在了时用", "重新开始", this::confirmRestart);
        if (status != null && !status.approved()) card.footer("本机还没拿到这个词库的密钥。");
    }

    /** 本机还没被允许：本机的核对码、去哪里允许，以及手边没有别的设备时的「重置密钥」。 */
    private void renderWaiting(LinearLayout target, Context context, CloudClipboardApi.Devices devices) {
        GroupCard card = GroupCard.add(sectionColumn(target, "设备"), "这台设备等待允许").withDividers(Ui.ROW_PADDING_H);
        card.note("在另一台已经在用词库同步的设备上打开「我的 → 词库同步」，那里会列出这台设备。核对那里显示的核对码和下面这串一致后，点「允许」。词库同步与云剪贴板分开允许。");
        LinearLayout code = LayoutPolicy.column(context);
        ViewPolicy.setSymmetricPaddingDp(context, code, Ui.ROW_PADDING_H, 14);
        code.addView(ViewPolicy.styledLabel(context, "本机核对码", 13, 400, ThemeColorPolicy.subText(context)));
        LinearLayout.LayoutParams digitsParams = LayoutPolicy.wrapParams();
        digitsParams.topMargin = DimensionPolicy.pixels(context, 4);
        code.addView(codeLabel(context, devices.code(), 22), digitsParams);
        card.addView(code);
        card.button("重置密钥", "手边没有别的设备时用", "重置", this::confirmReset);
        card.help("允许之后，这台设备的词和网盘上的词会合在一起，不会删掉任何一边的词。");
    }

    /** 等本机允许的其他设备：每台一行，名称、申请时间、核对码，行尾「拒绝」「允许」。 */
    private void renderPending(LinearLayout target, Context context, List<CloudClipboardApi.Device> pending) {
        if (pending.isEmpty()) return;
        GroupCard card = GroupCard.add(sectionColumn(target, "设备"), "等待允许").withDividers(Ui.ROW_PADDING_H);
        for (CloudClipboardApi.Device device : pending) {
            LinearLayout row = LayoutPolicy.row(context);
            ViewPolicy.setCenteredVertically(row);
            ViewPolicy.setPaddingDp(context, row, 16, 12, 12, 12);
            LinearLayout texts = LayoutPolicy.column(context);
            String name = DeviceNamePolicy.display(device.name());
            texts.addView(ViewPolicy.styledLabel(context, name, 15, 500, ThemeColorPolicy.text(context)));
            String when = CloudClipboardTimePolicy.relative(device.createdAt());
            if (!when.isEmpty()) {
                LinearLayout.LayoutParams metaParams = LayoutPolicy.wrapParams();
                metaParams.topMargin = DimensionPolicy.pixels(context, 2);
                texts.addView(ViewPolicy.styledLabel(context, when + "申请", 12, 400, ThemeColorPolicy.subText(context)), metaParams);
            }
            LinearLayout.LayoutParams codeParams = LayoutPolicy.wrapParams();
            codeParams.topMargin = DimensionPolicy.pixels(context, 4);
            texts.addView(codeLabel(context, device.code(), 15), codeParams);
            row.addView(texts, LayoutPolicy.weightedWrapParams(1f));
            row.addView(HomeButtonPolicy.tonalButton(context, "拒绝", "拒绝" + name, 500,
                () -> act(sync -> { sync.reject(device.deviceId()); return null; }, "已拒绝")), LayoutPolicy.rowGapParams(context, 8));
            row.addView(HomeButtonPolicy.tonalButton(context, "允许", "允许" + name, 500,
                () -> confirmApprove(device)), LayoutPolicy.rowGapParams(context, 8));
            card.addView(row);
        }
        card.footer("核对两台设备上的核对码一致再允许。不认识的设备点「拒绝」。");
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

    private void confirmApprove(CloudClipboardApi.Device device) {
        MessageDialog.confirm(requireContext(), "允许「" + DeviceNamePolicy.display(device.name()) + "」？",
            "核对两台设备上的核对码一致再允许：\n\n" + device.code()
                + "\n\n允许后，那台设备可以读取和改动这个网盘文件夹里同步的「我的词条」。",
            "允许", () -> act(sync -> { sync.approve(device.deviceId()); return null; }, "已允许"));
    }

    private void confirmRemove(CloudClipboardApi.Device device) {
        MessageDialog.confirm(requireContext(), "移除「" + DeviceNamePolicy.display(device.name()) + "」？",
            "移除会换一把新的密钥，只交给这台设备，网盘上的词库随即用新密钥重写。" + KEY_CHANGE + "移除之后，建议在网盘上换一个应用密码。",
            "移除", () -> act(sync -> { sync.remove(device.deviceId()); return null; }, "已移除"));
    }

    private void confirmReset() {
        MessageDialog.confirm(requireContext(), "重置词库同步的密钥？",
            "重置会换一把新的密钥，只交给这台设备。" + KEY_CHANGE,
            "重置", () -> act(sync -> { sync.reset(); return null; }, "已重置"));
    }

    private void confirmRestart() {
        MessageDialog.confirm(requireContext(), "以本机为准重新开始？",
            "网盘上的词库会按这台设备上的「我的词条」重新建一份。只在持有旧密钥的设备都不在了时用；其他设备允许之后，它们的词会合并进来，不会被删掉。",
            "重新开始", () -> act(sync -> { sync.restart(); return null; }, "已重新开始"));
    }

    private void confirmStop() {
        MessageDialog.confirm(requireContext(), "停止词库同步？",
            "会清除本机保存的网盘地址、账号和应用密码。网盘上的词库和这台设备上的词都不受影响，之后可以再设置。",
            "停止", () -> {
                generation++;
                KeyboardSheets.save(this, preferences -> preferences.remove(DictionarySyncPolicy.PREFERENCE), () -> {
                    MsToast.show(requireContext(), "已停止词库同步");
                    reload();
                }, this::reload);
            });
    }

    /** 先在工作线程捕获已存连接，再确认；未保存的草稿不能用来恢复。 */
    private void confirmTrustRemote() {
        if (busy) return;
        long owner = generation;
        Context application = requireContext().getApplicationContext();
        HostTask.run(this, DictionarySyncPage::savedConnection, saved -> {
            if (owner != generation) return;
            if (saved == null) {
                MsToast.show(requireContext(), "网盘设置没有读出来，请重新设置后再试");
                return;
            }
            MessageDialog.confirm(requireContext(), "重新信任网盘现状？",
                "服务器：" + saved.url() + "\n账号：" + saved.username()
                    + "\n\n只有在确认网盘上的词库文件是你自己清理或恢复的情况下继续。会遗忘本机待发送的改动和防回退记录，未上传的删除可能让词重新出现；本机现有词条会保留。",
                "重新信任", () -> trustRemote(owner, saved, application));
        });
    }

    /** 确认归属和已存三字段都没变时才恢复，之后的状态和设备读取仍由 work 的代次检查约束。 */
    private void trustRemote(long owner, SavedConnection saved, Context application) {
        if (owner != generation || busy) {
            MsToast.show(requireContext(), "网盘设置已更改，请重新确认恢复");
            return;
        }
        long request = ++generation;
        render();
        work(request, sync -> {
            SavedConnection current = savedConnection(application);
            if (request != generation || !saved.equals(current)) throw new SavedConnectionChanged();
            sync.trustRemote(saved.url(), saved.username(), saved.password());
            if (request != generation) throw new SavedConnectionChanged();
            return sync.run(DictionarySyncPolicy.TRIGGER_MANUAL, false, false);
        }, "已重新信任网盘并同步");
    }

    @Nullable private static SavedConnection savedConnection(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        JSONObject saved = preferences == null ? null : preferences.optJSONObject(DictionarySyncPolicy.PREFERENCE);
        if (saved == null) return null;
        String url = JsonPolicy.strictStringOrEmpty(saved.opt("url"));
        String username = JsonPolicy.strictStringOrEmpty(saved.opt("username"));
        String password = JsonPolicy.strictStringOrEmpty(saved.opt("password"));
        return url.isEmpty() ? null : new SavedConnection(url, username, password);
    }

    /**
     * 填写 WebDAV 网盘的服务器地址、账号和应用密码。三项按共享规则当场检查，不合规时在输入框下说明原因并禁用「测试连接」和「保存」。已经配置过网盘时应用密码框可以留空，表示保留已存的应用密码。「测试连接」只测不存；「保存」先测，通过了才写进偏好，没通过时对话框留着，原因写在输入框下面。
     */
    private void editWebDav(Config current) {
        boolean editing = current.configured();
        boolean keepPassword = editing && current.passwordSaved();
        InputDialog dialog = new InputDialog(requireContext(), "WebDAV 网盘",
            "服务器地址：坚果云填 https://dav.jianguoyun.com/dav/ 即可，水杉会在里面自动建 msime-dictionary 文件夹，不用自己先建；其他网盘填它给的 WebDAV 地址。可以和云剪贴板用同一个网盘账号，两边各自设置。应用密码是网盘为第三方应用生成的密码，不是登录密码，只存在本机，不随设置导出或账号同步。\n" + NOTE);
        EditText url = dialog.addField("服务器地址", editing ? current.url() : "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        EditText account = dialog.addField("账号", editing ? current.username() : "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);
        EditText password = dialog.addField(keepPassword ? "应用密码（留空保持不变）" : "应用密码", "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        // 上一次测试连接的结果是对哪一份草稿说的；改了任何一项就收起那句结果。
        AtomicReference<List<String>> tested = new AtomicReference<>();
        dialog.setValidator(values -> {
            String urlProblem = CloudClipboardRelayPolicy.webdavUrlProblem(values.get(0));
            String accountProblem = CloudClipboardRelayPolicy.usernameProblem(values.get(1));
            String passwordProblem = keepPassword && values.get(2).isEmpty()
                ? null : CloudClipboardRelayPolicy.passwordProblem(values.get(2));
            url.setError(values.get(0).isEmpty() ? null : urlProblem);
            account.setError(values.get(1).isEmpty() ? null : accountProblem);
            password.setError(values.get(2).isEmpty() ? null : passwordProblem);
            if (tested.get() != null && !tested.get().equals(values)) {
                tested.set(null);
                dialog.showStatus(null, false);
            }
            return urlProblem == null && accountProblem == null && passwordProblem == null;
        });
        dialog.keepOpenOnPrimary();
        dialog.addAction("测试连接", values -> probe(dialog, tested, values, keepPassword, false));
        dialog.setPrimary("保存", values -> probe(dialog, tested, values, keepPassword, true));
        dialog.show();
    }

    /** 在工作线程上跑一次测试连接；`save` 时通过了就接着保存，否则只把结果写在对话框里。 */
    private void probe(InputDialog dialog, AtomicReference<List<String>> tested, List<String> values,
            boolean keepPassword, boolean save) {
        String address = values.get(0);
        String account = values.get(1);
        String typed = values.get(2);
        dialog.setBusy(true);
        dialog.showStatus("正在测试连接……", false);
        HostTask.runNetwork(this, context -> {
            String password = typed.isEmpty() && keepPassword ? savedPassword(context) : typed;
            try {
                DictionaryWebDavSync.open(context).probe(address, account, password);
                return new Probe(null, password);
            } catch (DictionaryWebDavSync.Failure failure) {
                return new Probe(failure.code, "");
            }
        }, probe -> {
            if (!dialog.isShowing()) return;
            tested.set(values);
            if (probe == null || probe.code() != null) {
                dialog.setBusy(false);
                dialog.showStatus(probe == null ? "测试没能完成，请稍后重试"
                    : DictionarySyncPolicy.failureMessage(probe.code(), true), true);
                return;
            }
            if (!save) {
                dialog.setBusy(false);
                dialog.showStatus("连接正常，可以保存", false);
                return;
            }
            dialog.showStatus("连接正常，正在保存……", false);
            save(dialog, address, account, probe.password());
        });
    }

    /** 测试通过后写进偏好 `dictionary_sync_webdav`。写失败时对话框留着，可以再点「保存」。 */
    private void save(InputDialog dialog, String address, String account, String password) {
        // 在写偏好之前撤销旧任务，而不是等保存回调到达后才换代。
        generation++;
        KeyboardSheets.save(this, preferences -> {
            if (CloudClipboardRelayPolicy.webdavUrlProblem(address) != null
                    || CloudClipboardRelayPolicy.usernameProblem(account) != null
                    || CloudClipboardRelayPolicy.passwordProblem(password) != null)
                throw new JSONException("dictionary sync webdav is not valid");
            preferences.put(DictionarySyncPolicy.PREFERENCE, new JSONObject()
                .put("url", address).put("username", account).put("password", password));
        }, () -> {
            dialog.dismiss();
            MsToast.show(requireContext(), "已保存，开始同步「我的词条」");
            reload();
        }, () -> {
            dialog.setBusy(false);
            dialog.showStatus("保存失败，请重试", true);
            // 旧请求的回调已失效；写失败也须重新读取，不能把页面留在旧的 busy 状态。
            reload();
        });
    }

    /** 偏好里已存的应用密码；没有或读不了时为空串（测试连接随即报设置不完整）。读文件，在工作线程上调用。 */
    private static String savedPassword(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        JSONObject saved = preferences == null ? null : preferences.optJSONObject(DictionarySyncPolicy.PREFERENCE);
        return saved == null ? "" : JsonPolicy.strictStringOrEmpty(saved.opt("password"));
    }

    /** 偏好里的网盘；偏好读不了时为 null。读文件，在工作线程上调用。 */
    @Nullable private static Config readConfig(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        if (preferences == null) return null;
        JSONObject saved = preferences.optJSONObject(DictionarySyncPolicy.PREFERENCE);
        if (saved == null) return new Config("", "", false);
        return new Config(JsonPolicy.strictStringOrEmpty(saved.opt("url")),
            JsonPolicy.strictStringOrEmpty(saved.opt("username")),
            !JsonPolicy.strictStringOrEmpty(saved.opt("password")).isEmpty());
    }

    private static String relative(long millis) {
        try {
            return CloudClipboardTimePolicy.relative(Instant.ofEpochMilli(millis).toString());
        } catch (DateTimeException outOfRange) {
            return "";
        }
    }

    private static String shorten(String value) {
        return value.length() <= 32 ? value : value.substring(0, 31) + "…";
    }
}
