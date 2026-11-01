package app.msime.android.home;

import app.msime.android.ScreenCaptureProtection;
import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import app.msime.android.ImageViewPolicy;
import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.graphics.Typeface;
import android.text.InputType;
import android.view.View;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.CloudApi;
import app.msime.android.DeviceNamePolicy;
import app.msime.android.CloudClipboardApi;
import app.msime.android.CloudClipboardNativeDriver;
import app.msime.android.CloudClipboardRelay;
import app.msime.android.JsonPolicy;
import app.msime.android.R;
import app.msime.android.ViewPolicy;
import app.msime.android.clipboard.CloudClipboardRelayPolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.atomic.AtomicReference;
import app.msime.android.CloudClipboardTimePolicy;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 云剪贴板：主开关与保留时长一张卡；本机等待允许时是本机的核对码和说明；本机能读时依次是「等待允许」的其他设备（允许 / 拒绝）、「最近」列表（文本、置顶 · 设备 · 时间，行尾置顶与删除，右上角「清空」）和「设备」列表（移除、重置）。
 *
 * <p>文案如实写：端到端加密，文字在设备上加密后才上传，水杉云只保存密文，关闭即删除（`docs/cloud-clipboard-e2e.md`）。不用口令：新设备由已有设备允许，允许前两台设备上显示同一串 16 位核对码，让用户核对一致再点。移除设备和重置都会换一把只有本机持有的新密钥、删掉云端全部记录，其他设备都要在本机上重新允许，确认框里写明这一点。保留时长 1 / 7 / 30 天或一直保留，由服务端定期清理过期记录（{@link CloudClipboardApi}）。这一页从不读取系统剪贴板，点一条记录才把它复制过去。
 *
 * <p>只有真实账号有云剪贴板；没有登录时整页只显示一句说明。
 *
 * <p>最上面一张卡是「云剪贴板同步方式」：水杉账号、用户自建的中继（`docs/cloud-clipboard-relay.md`），或者用户自己的 WebDAV 网盘（`docs/cloud-clipboard-webdav.md`）。选了中继或网盘就只走它（{@link CloudClipboardRelay}；共享层把网盘也报成 `relay` 来源，下面的开关、列表和设备沿用中继那条路），不需要登录；出错也不退回水杉账号。地址和令牌在对话框里填好、校验通过后点「保存」才写进共享偏好（`cloud_clipboard_relay`），规则与共享设置页相同（{@link CloudClipboardRelayPolicy}）；令牌框打开时是空的，不回显已存的令牌。
 *
 * <p>WebDAV 网盘填服务器地址（网盘里的一个文件夹）、账号和应用密码，「测试连接」和「保存」都先在工作线程上跑共享层的测试连接（{@link CloudClipboardRelay#probeWebDav}），通过了「保存」才写进 `cloud_clipboard_webdav`；没通过时对话框和填好的内容都留着，原因写在输入框下面。中继和网盘只能二选一：保存其中一个就删掉另一个，改回水杉账号两个都删。应用密码框同样不回显，已经保存过时留空表示保留原来的。
 */
public final class CloudClipboardPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("设置", "记录", "设备");
    }

    private static final String DESCRIPTION = "在你登录的设备之间同步，端到端加密，水杉云只保存密文，关闭即删除";
    private static final String KEY_CHANGE = "云端现有的记录会全部删除，其他设备都要在这台设备上重新允许后才能继续同步。";
    private static final String RELAY_DESCRIPTION = "在你的设备之间经自建中继同步，端到端加密，中继只保存密文，关闭即删除";
    private static final String WEBDAV_DESCRIPTION = "在你的设备之间经 WebDAV 网盘同步，端到端加密，网盘只保存密文，关闭即删除";
    /** WebDAV 网盘的说明，与共享设置页的同一段相同；填写框里写，设置好之后收进卡片组标题的「?」。 */
    private static final String WEBDAV_NOTE = "内容在本机加密，网盘只保存密文；打开面板或复制时同步，没有实时推送；坚果云免费账号每 30 分钟 600 次请求与其他应用共用；建议使用专用账号或专用应用密码，移除设备后更换应用密码；已删除的密文可能留在网盘回收站。";
    private static final List<String> RETENTION_LABELS = List.of("1 天", "7 天", "30 天", "一直");
    private record Loaded(CloudClipboardApi api, CloudClipboardApi.Overview overview) {}
    /** 云剪贴板的同步方式。 */
    private enum Mode { ACCOUNT, RELAY, WEBDAV }
    /** 偏好里的同步方式：`url` 是中继或网盘的地址，`username` 只有网盘有；令牌和应用密码只记有没有（`secretSaved`），不留在页面上。 */
    private record Relay(Mode mode, String url, String username, boolean secretSaved) {
        boolean configured() { return mode != Mode.ACCOUNT; }
    }
    /** 一次「测试连接」的结果：`code` 为 null 表示通过；`password` 是实际测试用的应用密码（留空保持不变时是已存的那个），通过后「保存」原样写进偏好。 */
    private record Probe(@Nullable String code, String password) {}

    @Nullable private CloudClipboardApi.Overview overview;
    @Nullable private CloudClipboardApi loadedApi;
    private boolean busy;
    private boolean signedOut;
    private long generation;
    /** 读到的同步方式；还没读到或偏好读不了时为 null，这时不画「同步方式」那张卡。 */
    @Nullable private Relay relay;
    @Override protected void onContentReady() {
        render();
    }

    @Override protected boolean shouldReloadWhenVisible() {
        // 写操作完成后会自行重读；此时抢跑读取可能先于写操作返回，留下旧列表。
        return !busy;
    }

    @Override protected void onContentDestroyed() {
        generation++;
        overview = null;
        loadedApi = null;
        busy = false;
        signedOut = false;
        relay = null;
    }

    @Override protected void reload() {
        Context application = requireContext().getApplicationContext();
        long request = ++generation;
        busy = true;
        overview = null;
        loadedApi = null;
        signedOut = false;
        render();
        HostTask.run(this, CloudClipboardPage::readRelay, read -> {
            if (request != generation) return;
            relay = read;
            render();
        });
        NetworkTask.run(this, () -> {
            // 配置了中继就只走中继，以共享层的判断为准；读不了偏好或中继配置不合规时抛出，不退回水杉账号。
            if (CloudClipboardRelay.selected(CloudClipboardRelay.preferencesDirectory(application))) {
                CloudClipboardApi api = new CloudClipboardApi(CloudClipboardRelay.open(application));
                return new Loaded(api, api.load());
            }
            CloudApi cloud = new CloudApi(application);
            String userId = cloud.currentAccountSessionId();
            CloudClipboardApi api = new CloudClipboardApi(application, userId, cloud.forAccountSession(userId),
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
                    ? failureMessage(failure, "云剪贴板没有读出来，请稍后再试", viaWebDav()) : "云剪贴板没有读出来，请稍后再试");
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
                    ? failureMessage(failure, "没有完成，请稍后再试", viaWebDav()) : "没有完成，请稍后再试");
                overview = null;
                loadedApi = null;
                render();
                return;
            }
            if (done != null) MsToast.show(requireContext(), done);
            reload();
        });
    }

    /**
     * 失败时给人看的一句话；驱动、账号服务或中继的错误码在 {@link CloudApi.Failure#code} 里。先看错误码再看是不是没连上：驱动和中继报的错 `status` 也是 0（例如本机还在等待允许），只按 `status` 会都说成连不上服务器。`webdav` 为真时走的是 WebDAV 网盘，中继的码说成网盘、账号和应用密码。
     */
    static String failureMessage(CloudApi.Failure failure, String fallback, boolean webdav) {
        String relayMessage = CloudClipboardRelayPolicy.failureMessage(failure.code, webdav);
        if (relayMessage != null) return relayMessage;
        return switch (failure.code) {
            case "clipboard_key_conflict" -> "另一台设备刚刚改了云剪贴板，请稍后再试";
            case "clipboard_keystore_conflict" -> "这台设备的云剪贴板身份刚被更换（例如退出后重新登录），请稍后再试";
            case "clipboard_device_limit" -> "这个账号登记的设备已满 20 台，请先移除不用的设备";
            case "clipboard_sync_disabled" -> "云剪贴板未开启";
            case "clipboard_device_pending" -> "这台设备还在等待允许";
            case "not_found" -> "那台设备已经不在了，请刷新后再试";
            case "session_changed" -> "登录已改变，请重新打开页面";
            default -> failure.network() ? "连不上服务器，请检查网络后重试" : fallback;
        };
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        Context context = target.getContext();
        target.removeAllViews();
        renderSyncMode(target);

        if (signedOut) {
            GroupCard card = GroupCard.add(target, null);
            card.note("登录水杉账号后，复制的文字可以在你的设备之间同步。" + DESCRIPTION
                + "。不想登录的话，可以在上面把同步方式改成自建中继或 WebDAV 网盘。");
            return;
        }

        CloudClipboardApi.Overview current = overview;
        CloudClipboardApi.Page page = current == null ? null : current.page();
        GroupCard settings = GroupCard.add(sectionColumn(target, "设置"), null).withDividers(Ui.ROW_PADDING_H);
        GroupCard.Row toggle = settings.toggle("云剪贴板", null,
            page != null && page.enabled(), this::setEnabled).help(viaWebDav() ? WEBDAV_DESCRIPTION : viaRelay() ? RELAY_DESCRIPTION : DESCRIPTION);
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

    /** 「云剪贴板同步方式」：水杉账号、自建中继或 WebDAV 网盘；选了中继或网盘时列出地址和凭据，点了都打开同一个填写框。 */
    private void renderSyncMode(LinearLayout target) {
        Relay current = relay;
        if (current == null) return;
        GroupCard card = GroupCard.add(sectionColumn(target, "设置"), null).withDividers(Ui.ROW_PADDING_H);
        // 这张卡片没有组标题，选了 WebDAV 时网盘的注意事项也并进「同步方式」这一行的「?」。
        card.nav("同步方式", null,
            modeLabel(current.mode()), () -> pickSyncMode(current)).help("自建中继和 WebDAV 网盘都不需要登录水杉账号：内容仍在本机加密，中继或网盘只保存密文，新设备照样要在已有设备上核对后允许"
                + (current.mode() == Mode.WEBDAV ? "\n\n" + WEBDAV_NOTE : ""));
        switch (current.mode()) {
            case RELAY -> {
                card.nav("中继地址", null, shorten(current.url()), () -> editRelay(current));
                card.nav("中继令牌", null, current.secretSaved() ? "已填写" : "未填写", () -> editRelay(current));
                card.footer("云剪贴板现在只经这个中继同步，中继出错时也不会改用水杉账号。换了地址或令牌后，这台设备要在中继上重新登记，并在其他设备上重新允许。");
            }
            case WEBDAV -> {
                card.nav("服务器地址", null, shorten(current.url()), () -> editWebDav(current));
                card.nav("账号", null, shorten(current.username()), () -> editWebDav(current));
                card.nav("应用密码", null, current.secretSaved() ? "已填写" : "未填写", () -> editWebDav(current));
                card.footer("云剪贴板现在经 WebDAV 网盘 " + current.url() + " 同步，网盘出错时也不会改用水杉账号。换了地址或账号后，本机要重新登记，并在其他设备上重新允许；换应用密码不用。");
            }
            case ACCOUNT -> { }
        }
    }

    private static String modeLabel(Mode mode) {
        return switch (mode) {
            case ACCOUNT -> "水杉账号";
            case RELAY -> "自建中继";
            case WEBDAV -> "WebDAV";
        };
    }

    private void pickSyncMode(Relay current) {
        OptionSheet sheet = new OptionSheet(requireContext(), "云剪贴板同步方式", null);
        sheet.option("水杉账号", current.mode() == Mode.ACCOUNT, () -> {
            if (current.configured()) confirmAccount(current);
        });
        sheet.option("自建中继", current.mode() == Mode.RELAY, () -> {
            if (current.mode() != Mode.RELAY) editRelay(current);
        });
        sheet.option("WebDAV", current.mode() == Mode.WEBDAV, () -> {
            if (current.mode() != Mode.WEBDAV) editWebDav(current);
        });
        sheet.show();
    }

    /**
     * 填写中继地址和令牌。两项都按共享规则当场检查，不合规时在输入框下说明原因并禁用「保存」。已经配置过中继时令牌框可以留空，表示保留已存的令牌。保存时删掉 WebDAV 网盘，两者只能二选一。
     */
    private void editRelay(Relay current) {
        boolean editing = current.mode() == Mode.RELAY;
        boolean keepToken = editing && current.secretSaved();
        InputDialog dialog = new InputDialog(requireContext(), "自建中继",
            "例如 https://relay.example.com/msime。必须是 HTTPS，证书要由公网信任的机构签发（自签证书和私有 CA 连不上）；HTTP 只用于本机测试（127.0.0.1）。令牌由中继管理员生成，每个令牌对应中继上一份独立的云剪贴板；它只存在本机，不随设置导出或账号同步。");
        EditText url = dialog.addField("中继地址", editing ? current.url() : "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        EditText token = dialog.addField(keepToken ? "中继令牌（留空保持不变）" : "中继令牌", "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        dialog.setValidator(values -> {
            String urlProblem = CloudClipboardRelayPolicy.urlProblem(values.get(0));
            String tokenProblem = keepToken && values.get(1).isEmpty()
                ? null : CloudClipboardRelayPolicy.tokenProblem(values.get(1));
            url.setError(values.get(0).isEmpty() ? null : urlProblem);
            token.setError(values.get(1).isEmpty() ? null : tokenProblem);
            return urlProblem == null && tokenProblem == null;
        });
        dialog.setPrimary("保存", values -> KeyboardSheets.save(this, preferences -> {
            String address = values.get(0);
            String secret = values.get(1);
            if (secret.isEmpty() && keepToken) {
                JSONObject saved = preferences.optJSONObject(CloudClipboardRelayPolicy.PREFERENCE);
                secret = saved == null ? "" : JsonPolicy.strictStringOrEmpty(saved.opt("token"));
            }
            if (CloudClipboardRelayPolicy.urlProblem(address) != null
                    || CloudClipboardRelayPolicy.tokenProblem(secret) != null)
                throw new JSONException("cloud clipboard relay is not valid");
            preferences.remove(CloudClipboardRelayPolicy.WEBDAV_PREFERENCE);
            preferences.put(CloudClipboardRelayPolicy.PREFERENCE,
                new JSONObject().put("url", address).put("token", secret));
        }, () -> {
            MsToast.show(requireContext(), "已保存，云剪贴板改走自建中继");
            reload();
        }, this::reload));
        dialog.show();
    }

    /**
     * 填写 WebDAV 网盘的服务器地址、账号和应用密码。三项按共享规则当场检查，不合规时在输入框下说明原因并禁用「测试连接」和「保存」。已经配置过网盘时应用密码框可以留空，表示保留已存的应用密码。「测试连接」只测不存；「保存」先测，通过了才写进偏好，没通过时对话框留着，原因写在输入框下面。
     */
    private void editWebDav(Relay current) {
        boolean editing = current.mode() == Mode.WEBDAV;
        boolean keepPassword = editing && current.secretSaved();
        InputDialog dialog = new InputDialog(requireContext(), "WebDAV 网盘",
            "服务器地址：坚果云填 https://dav.jianguoyun.com/dav/ 即可，水杉会在里面自动建 msime-clipboard 文件夹，不用自己先建；其他网盘填它给的 WebDAV 地址。应用密码是网盘为第三方应用生成的密码，不是登录密码。\n" + WEBDAV_NOTE);
        EditText url = dialog.addField("服务器地址", editing ? current.url() : "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        EditText account = dialog.addField("账号", editing ? current.username() : "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);
        EditText password = dialog.addField(keepPassword ? "应用密码（留空保持不变）" : "应用密码", "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        // 上一次测试连接的结果是对哪一份草稿说的；改了任何一项就收起那句结果，免得「连接正常」留在改过的内容下面。
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
        dialog.addAction("测试连接", values -> probeWebDav(dialog, tested, values, keepPassword, false));
        dialog.setPrimary("保存", values -> probeWebDav(dialog, tested, values, keepPassword, true));
        dialog.show();
    }

    /** 在工作线程上跑一次测试连接；`save` 时通过了就接着保存，否则只把结果写在对话框里。 */
    private void probeWebDav(InputDialog dialog, AtomicReference<List<String>> tested, List<String> values,
            boolean keepPassword, boolean save) {
        String address = values.get(0);
        String account = values.get(1);
        String typed = values.get(2);
        dialog.setBusy(true);
        dialog.showStatus("正在测试连接……", false);
        HostTask.runNetwork(this, context -> {
            String password = typed.isEmpty() && keepPassword ? savedWebDavPassword(context) : typed;
            try {
                CloudClipboardRelay.probeWebDav(context, address, account, password);
                return new Probe(null, password);
            } catch (CloudApi.Failure failure) {
                return new Probe(failure.code, "");
            }
        }, probe -> {
            if (!dialog.isShowing()) return;
            tested.set(values);
            if (probe == null || probe.code() != null) {
                dialog.setBusy(false);
                String message = probe == null ? null : CloudClipboardRelayPolicy.failureMessage(probe.code(), true);
                dialog.showStatus(message == null ? "测试没能完成，请稍后重试" : message, true);
                return;
            }
            if (!save) {
                dialog.setBusy(false);
                dialog.showStatus("连接正常，可以保存", false);
                return;
            }
            dialog.showStatus("连接正常，正在保存……", false);
            saveWebDav(dialog, address, account, probe.password());
        });
    }

    /** 测试通过后写进偏好：写入 `cloud_clipboard_webdav`、删掉中继。写失败时对话框留着，可以再点「保存」。 */
    private void saveWebDav(InputDialog dialog, String address, String account, String password) {
        KeyboardSheets.save(this, preferences -> {
            if (CloudClipboardRelayPolicy.webdavUrlProblem(address) != null
                    || CloudClipboardRelayPolicy.usernameProblem(account) != null
                    || CloudClipboardRelayPolicy.passwordProblem(password) != null)
                throw new JSONException("cloud clipboard webdav is not valid");
            preferences.remove(CloudClipboardRelayPolicy.PREFERENCE);
            preferences.put(CloudClipboardRelayPolicy.WEBDAV_PREFERENCE, new JSONObject()
                .put("url", address).put("username", account).put("password", password));
        }, () -> {
            dialog.dismiss();
            MsToast.show(requireContext(), "已保存，云剪贴板改走 WebDAV 网盘");
            reload();
        }, () -> {
            dialog.setBusy(false);
            dialog.showStatus("保存失败，请重试", true);
        });
    }

    /** 偏好里已存的 WebDAV 应用密码；没有或读不了时为空串（测试连接随即报设置不完整）。读文件，在工作线程上调用。 */
    private static String savedWebDavPassword(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        JSONObject saved = preferences == null ? null
            : preferences.optJSONObject(CloudClipboardRelayPolicy.WEBDAV_PREFERENCE);
        return saved == null ? "" : JsonPolicy.strictStringOrEmpty(saved.opt("password"));
    }

    /** 改回水杉账号：清掉偏好里的中继和 WebDAV 网盘，中继或网盘上的内容和本机的中继密钥串都不动。 */
    private void confirmAccount(Relay current) {
        String cleared = current.mode() == Mode.WEBDAV
            ? "会清除本机保存的网盘地址、账号和应用密码，网盘上的内容不受影响。"
            : "会清除本机保存的中继地址和令牌，中继上的内容不受影响。";
        ScreenCaptureProtection.show(new MaterialAlertDialogBuilder(requireContext())
            .setTitle("改回水杉账号？")
            .setMessage(cleared + "之后云剪贴板经水杉账号同步，要登录才能用。")
            .setNegativeButton("取消", null)
            .setPositiveButton("改回", (dialog, which) -> KeyboardSheets.save(this, preferences -> {
                    preferences.remove(CloudClipboardRelayPolicy.PREFERENCE);
                    preferences.remove(CloudClipboardRelayPolicy.WEBDAV_PREFERENCE);
                }, () -> {
                    MsToast.show(requireContext(), "已改回水杉账号");
                    reload();
                }, this::reload))
            .create());
    }

    /** 偏好里的同步方式；偏好读不了时为 null。网盘优先，与 {@link CloudClipboardRelay#binding} 相同（两者同时配置的文档共享层不认）。读文件，在工作线程上调用。 */
    @Nullable private static Relay readRelay(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        if (preferences == null) return null;
        JSONObject webdav = preferences.optJSONObject(CloudClipboardRelayPolicy.WEBDAV_PREFERENCE);
        if (webdav != null) {
            String url = JsonPolicy.strictStringOrEmpty(webdav.opt("url"));
            if (CloudClipboardRelayPolicy.configured(url))
                return new Relay(Mode.WEBDAV, url, JsonPolicy.strictStringOrEmpty(webdav.opt("username")),
                    !JsonPolicy.strictStringOrEmpty(webdav.opt("password")).isEmpty());
        }
        JSONObject saved = preferences.optJSONObject(CloudClipboardRelayPolicy.PREFERENCE);
        if (saved == null) return new Relay(Mode.ACCOUNT, "", "", false);
        String url = JsonPolicy.strictStringOrEmpty(saved.opt("url"));
        return new Relay(CloudClipboardRelayPolicy.configured(url) ? Mode.RELAY : Mode.ACCOUNT, url, "",
            !JsonPolicy.strictStringOrEmpty(saved.opt("token")).isEmpty());
    }

    /** 下面的开关、列表和设备是不是中继（或 WebDAV 网盘）上的；同步方式还没读到时按水杉账号的说法。 */
    private boolean viaRelay() {
        Relay current = relay;
        return current != null && current.configured();
    }

    /** 走的是不是 WebDAV 网盘：说法里的「中继」换成「网盘」。 */
    private boolean viaWebDav() {
        Relay current = relay;
        return current != null && current.mode() == Mode.WEBDAV;
    }

    /** 存内容的那一方，写进说明和确认框。 */
    private String store() {
        return viaWebDav() ? "网盘" : viaRelay() ? "中继" : "水杉云";
    }

    private static String shorten(String value) {
        return value.length() <= 32 ? value : value.substring(0, 31) + "…";
    }

    /** 本机还没被允许：本机的核对码、去哪里允许，以及没有其他设备可用时的「重置」。 */
    private void renderWaiting(LinearLayout target, Context context, CloudClipboardApi.Devices devices) {
        GroupCard card = GroupCard.add(sectionColumn(target, "设备"), "这台设备等待允许").withDividers(Ui.ROW_PADDING_H);
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
        card.help("允许之后这里会显示同步的内容。");
    }

    /** 等本机允许的其他设备：名称、登记时间、核对码，行尾「拒绝」「允许」。 */
    private void renderPendingDevices(LinearLayout target, Context context, List<CloudClipboardApi.Device> pending) {
        if (pending.isEmpty()) return;
        GroupCard card = GroupCard.add(sectionColumn(target, "设备"), "等待允许").withDividers(Ui.ROW_PADDING_H);
        for (CloudClipboardApi.Device device : pending) {
            LinearLayout row = LayoutPolicy.row(context);
            ViewPolicy.setCenteredVertically(row);
            ViewPolicy.setPaddingDp(context, row, 16, 12, 12, 12);
            LinearLayout texts = LayoutPolicy.column(context);
            texts.addView(ViewPolicy.styledLabel(context, DeviceNamePolicy.display(device.name()), 15, 500, ThemeColorPolicy.text(context)));
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
            row.addView(HomeButtonPolicy.tonalButton(context, "拒绝", "拒绝" + DeviceNamePolicy.display(device.name()), 500,
                () -> reject(device)), LayoutPolicy.rowGapParams(context, 8));
            row.addView(HomeButtonPolicy.tonalButton(context, "允许", "允许" + DeviceNamePolicy.display(device.name()), 500,
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
        sectionColumn(target, "记录").addView(header, headerParams);

        GroupCard list = GroupCard.add(sectionColumn(target, "记录"), null).withDividers(Ui.ROW_PADDING_H);
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
        card.button("重置云剪贴板", "换一把新密钥，所有设备重新允许", "重置", this::confirmReset);
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
        String when = CloudClipboardTimePolicy.relative(item.updatedAt());
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
    private void setEnabled(boolean enabled) {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        if (enabled) {
            mutate(() -> { api.setEnabled(true); return null; }, null);
            return;
        }
        ScreenCaptureProtection.show(new MaterialAlertDialogBuilder(requireContext())
            .setTitle("关闭云剪贴板？")
            .setMessage("关闭后，保存在" + store() + "上的全部记录都会删除，所有设备的登记也一起删除，再次开启时其他设备要重新允许。")
            .setNegativeButton("取消", (dialog, which) -> render())
            .setOnCancelListener(dialog -> render())
            .setPositiveButton("关闭", (dialog, which) -> mutate(() -> { api.setEnabled(false); return null; }, "已关闭"))
            .create());
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
        MessageDialog.confirm(requireContext(), "清空云剪贴板？", "所有设备上同步的记录都会删除，置顶的也不例外。",
            "清空", () -> mutate(() -> { api.clear(); return null; }, "已清空"));
    }

    private void confirmApprove(CloudClipboardApi.Device device) {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        MessageDialog.confirm(requireContext(), "允许「" + DeviceNamePolicy.display(device.name()) + "」？",
            "核对两台设备上的核对码一致再允许：\n\n" + device.code()
                + "\n\n允许后，那台设备可以读取和上传" + (viaWebDav() ? "这个网盘文件夹里" : viaRelay() ? "这个中继令牌下" : "这个账号") + "的云剪贴板。",
            "允许", () -> mutate(() -> api.approve(device.deviceId()), "已允许"));
    }

    private void reject(CloudClipboardApi.Device device) {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        mutate(() -> api.reject(device.deviceId()), "已拒绝");
    }

    private void confirmRemove(CloudClipboardApi.Device device) {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        MessageDialog.confirm(requireContext(), "移除「" + DeviceNamePolicy.display(device.name()) + "」？",
            "移除会换一把新的密钥，只交给这台设备。" + KEY_CHANGE,
            "移除", () -> mutate(() -> api.remove(device.deviceId()), "已移除"));
    }

    private void confirmReset() {
        CloudClipboardApi api = loadedApi;
        if (api == null) return;
        MessageDialog.confirm(requireContext(), "重置云剪贴板？",
            "重置会换一把新的密钥，只交给这台设备。" + KEY_CHANGE,
            "重置", () -> mutate(() -> api.reset(), "已重置"));
    }
}
