package app.msime.android.home;

import app.msime.android.ScreenCaptureProtection;
import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.provider.DocumentsContract;
import android.widget.LinearLayout;
import androidx.activity.result.ActivityResult;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.core.content.FileProvider;
import androidx.biometric.BiometricManager;
import androidx.biometric.BiometricPrompt;
import androidx.core.content.ContextCompat;
import app.msime.android.AccountFailureText;
import app.msime.android.SettingsResetAuthenticationPolicy;
import app.msime.android.SystemAuthenticators;
import app.msime.android.AccountStore;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.AppVersionPolicy;
import app.msime.android.DeviceDataApi;
import app.msime.android.DiagnosticsApi;
import app.msime.android.RustAccount;
import app.msime.android.FilePolicy;
import app.msime.android.McpUploadSwitchPolicy;
import app.msime.android.NativeClient;
import app.msime.android.JsonPolicy;
import app.msime.android.NumberPolicy;
import app.msime.android.PreferencesRevisionPolicy;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import app.msime.android.TimestampDisplayPolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.io.File;
import java.io.IOException;
import java.time.Instant;
import java.time.LocalDate;
import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.UUID;
import java.util.function.Consumer;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 开发者选项：MCP 开发者访问（把选定的日志打包上传一次到水杉云，供开发者用 MCP 读取）、调试开关、导出诊断包和重置所有设置。
 *
 * <p>P19：MCP 上传默认关闭，每次上传（包括重新上传）都要在「确认上传」组里点一次「上传」；「输入事件」默认不勾选。上传由 Rust 从本机记录现读现建（与导出诊断包同一份 bundle），配置快照已脱敏，输入事件和性能数据只有时间、耗时和事件种类，见 {@link DiagnosticsApi}。是否已上传、远程地址和令牌是服务端状态，每次进页从云端读回；完整令牌只在上传或重新生成的那一次回答里出现，只留在本页内存里。
 *
 * <p>页面记下两个账号编号：打开时登录的账号（`cloudAccountId`，没有登录为空串），每次操作前核对它没变；快照所属的账号（`cloudSessionId`，登录账号或匿名账号，上传后换成 Rust 交回的那一个），查看、换令牌和删除都带着它，Rust 发现会话已经不是它时报 account_identity_changed。
 *
 * <p>调试开关、日志级别、「记录输入日志」和 MCP 上传的内容与保留时长只在本机，存在 {@link AndroidLocalSettings}（`platform.android.developer.*`），不进共享偏好、不同步；「重置所有设置」同时把共享偏好和这份本地设置恢复为默认值。
 */
public final class DeveloperPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("日志共享", "调试", "数据");
    }

    private static final String PLATFORM = "android";
    private static final String DIAGNOSTICS_CACHE = "diagnostics";
    private static final DateTimeFormatter CLOCK = DateTimeFormatter.ofPattern("HH:mm", Locale.ROOT);
    private static final DateTimeFormatter DAY_CLOCK = DateTimeFormatter.ofPattern("M月d日 HH:mm", Locale.ROOT);

    @Nullable private JSONObject snapshot;
    private AndroidLocalSettings.Snapshot local = AndroidLocalSettings.defaults();
    private DiagnosticsApi.State cloud = DiagnosticsApi.State.EMPTY;
    private boolean cloudLoaded;
    private boolean confirming;
    private boolean busy;
    @Nullable private BiometricPrompt resetAuthentication;
    private boolean exportAfterPicking;
    private String exportDirectoryName = "未选择";
    private long generation;
    private String cloudAccountId = "";
    private String cloudSessionId = "";
    /** 本次会话里拿到的完整令牌；只在内存里，离开页面就丢。 */
    @Nullable private String freshToken;
    private final Set<Integer> expanded = new HashSet<>();

    private record LoadedCloud(DiagnosticsApi.State state, String accountId, String sessionId) {}

    private final ActivityResultLauncher<Intent> chooseExportDirectory = registerForActivityResult(
        new ActivityResultContracts.StartActivityForResult(), this::onExportDirectoryPicked);

    @Override public void onCreate(@Nullable Bundle state) {
        super.onCreate(state);
        exportAfterPicking = state != null && state.getBoolean("exportAfterPicking");
    }

    @Override public void onSaveInstanceState(@NonNull Bundle state) {
        state.putBoolean("exportAfterPicking", exportAfterPicking);
        super.onSaveInstanceState(state);
    }

    @Override protected void onContentReady() {
        render();
    }

    @Override protected void onContentDestroyed() {
        if (resetAuthentication != null) resetAuthentication.cancelAuthentication();
        resetAuthentication = null;
        generation++;
        cloudAccountId = "";
        cloudSessionId = "";
        // Work results are fenced to the old view and will be dropped after recreation. Do not
        // leave the rebuilt page permanently disabled, and never retain a full access token past
        // the view that displayed it.
        busy = false;
        confirming = false;
        freshToken = null;
    }

    @Override protected void reload() {
        if (getView() == null) return;
        long request = ++generation;
        cloud = DiagnosticsApi.State.EMPTY;
        cloudLoaded = false;
        cloudAccountId = "";
        cloudSessionId = "";
        render();
        HostTask.run(this, context -> new Object[] {HostStore.loadPreferences(context),
            AndroidLocalSettings.load(context), exportDirectoryName(context)}, loaded -> {
            if (request != generation || getView() == null) return;
            snapshot = loaded[0] instanceof JSONObject value ? value : null;
            if (loaded[1] instanceof AndroidLocalSettings.Snapshot settings) local = settings;
            if (loaded[2] instanceof String name) exportDirectoryName = name;
            render();
        });
        // 读云端状态是一次 HTTP 请求，走网络线程；放在共享存储的串行线程上时，网络慢的那几十秒里开关的保存都排在它后面。
        HostTask.runNetwork(this, context -> {
            try {
                String accountId = DeviceDataApi.currentUserId(context);
                String sessionId = DiagnosticsApi.uploaderId(context);
                DiagnosticsApi.State state = DiagnosticsApi.state(context, sessionId);
                RustAccount.requireUser(context, accountId);
                return new LoadedCloud(state, accountId, sessionId);
            } catch (AccountStore.Failure failure) {
                return null;
            }
        }, result -> {
            if (request != generation || getView() == null) return;
            LoadedCloud loaded = result;
            cloudLoaded = loaded != null;
            if (loaded != null) {
                cloud = loaded.state();
                cloudAccountId = loaded.accountId();
                cloudSessionId = loaded.sessionId();
            }
            render();
        });
    }

    /** 把诊断页的每个操作限制在加载页面时登录的账号；匿名页也拒绝账号后来变成真实账号。 */
    private static void requirePage(Context context, String expectedAccountId) throws AccountStore.Failure {
        RustAccount.requireUser(context, expectedAccountId);
    }

    // ---- 渲染 ----

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        AndroidLocalSettings.Snapshot settings = local;
        DiagnosticsApi.Retention retention = DiagnosticsApi.Retention.fromWire(
            settings.choice(AndroidLocalSettings.MCP_RETENTION));
        DiagnosticsApi.Snapshot uploaded = cloud.snapshot();

        GroupCard mcp = GroupCard.add(sectionColumn(target, "日志共享"), "MCP 开发者访问");
        // 等确认、已上传是当前状态，留在副标题；这项做什么收进「?」。
        CharSequence mcpSubtitle;
        if (confirming) {
            mcpSubtitle = "请在下方确认后上传";
        } else if (uploaded != null) {
            mcpSubtitle = "已上传 · " + when(uploaded.createdAt()) + " · " + remaining(uploaded.expiresAt());
        } else {
            mcpSubtitle = null;
        }
        GroupCard.Row mcpRow = mcp.toggle("上传日志供开发者通过 MCP 读取", mcpSubtitle,
            McpUploadSwitchPolicy.checked(uploaded != null, confirming),
            on -> {
                switch (McpUploadSwitchPolicy.onToggle(on, confirming)) {
                    case START_CONFIRM -> {
                        confirming = true;
                        render();
                    }
                    case CANCEL_CONFIRM -> {
                        confirming = false;
                        render();
                    }
                    case DELETE -> confirmDelete();
                }
            });
        mcpRow.help("把下方选定的日志打包上传一次到水杉云，开发者在自己电脑上读取这份快照，手机无需保持在线，不会读取你输入的文字");
        mcpRow.setEnabled(!busy && snapshot != null);
        mcp.nav("保留时长", null, retention.label(), () -> showRetentionSheet(retention));

        if (confirming) {
            DiagnosticsApi.Include include = include(settings);
            GroupCard confirm = GroupCard.add(sectionColumn(target, "日志共享"), "确认上传");
            String summary = include.any()
                ? categories(include) + " · " + retention.label() + "后自动删除，可随时删除"
                : "请至少在下方选择一类日志";
            GroupCard.Row send = confirm.button("将上传并允许开发者读取", summary, "上传", this::upload);
            send.setEnabled(include.any() && !busy);
            confirm.button("暂不授权", null, "取消", () -> {
                confirming = false;
                render();
            });
        }

        GroupCard logs = GroupCard.add(sectionColumn(target, "日志共享"), "可访问的日志");
        uploadToggle(logs, "崩溃日志", "崩溃堆栈与引擎错误", settings, AndroidLocalSettings.MCP_CRASH_LOGS);
        uploadToggle(logs, "性能日志", "候选耗时与内存占用", settings, AndroidLocalSettings.MCP_PERFORMANCE_LOGS);
        uploadToggle(logs, "输入事件", "只含按键时序与事件种类，不含文字内容", settings, AndroidLocalSettings.MCP_INPUT_EVENTS);
        uploadToggle(logs, "配置快照", "当前设置与已安装词库", settings, AndroidLocalSettings.MCP_CONFIG_SNAPSHOT);

        if (uploaded != null) {
            renderUploaded(target, uploaded);
            renderAccesses(target);
            GroupCard remove = GroupCard.add(sectionColumn(target, "日志共享"), null);
            remove.button("删除云端日志", "云端快照和令牌立即删除，开发者无法再读取", "删除", this::confirmDelete)
                .setEnabled(!busy);
        } else if (!cloudLoaded && snapshot != null) {
            mcp.note("暂时读不到云端快照的状态，联网后返回本页会再读一次");
        }

        GroupCard debug = GroupCard.add(sectionColumn(target, "调试"), "调试");
        boolean loggingEnabled = settings.bool(AndroidLocalSettings.DEVELOPER_LOGGING_ENABLED);
        debug.toggle("日志总开关", null, loggingEnabled,
            on -> edit(AndroidLocalSettings.DEVELOPER_LOGGING_ENABLED, on)).help("关闭后不显示调试信息，也不记录输入日志");
        GroupCard.Row overlay = debug.toggle("显示调试信息", null,
            settings.bool(AndroidLocalSettings.DEVELOPER_DEBUG_OVERLAY),
            on -> edit(AndroidLocalSettings.DEVELOPER_DEBUG_OVERLAY, on)).help("在候选栏显示引擎耗时与词频");
        overlay.setEnabled(loggingEnabled);
        GroupCard.Row inputLog = debug.toggle("记录输入日志", null, settings.bool(AndroidLocalSettings.DEVELOPER_INPUT_LOG),
            on -> edit(AndroidLocalSettings.DEVELOPER_INPUT_LOG, on)).help("仅保存在本机，不会上传");
        inputLog.setEnabled(loggingEnabled);
        String level = settings.choice(AndroidLocalSettings.DEVELOPER_LOG_LEVEL);
        GroupCard.Row levelRow = debug.nav("日志级别", null, levelLabel(level),
            snapshot == null ? null : () -> showLevelSheet(level)).help("只影响系统日志（logcat），导出的诊断包不受它影响");
        levelRow.setEnabled(loggingEnabled && snapshot != null);

        GroupCard data = GroupCard.add(sectionColumn(target, "数据"), "数据");
        data.nav("默认导出文件夹", exportDirectoryName, null, () -> pickExportDirectory(false))
            .setEnabled(!busy && snapshot != null);
        data.button("导出诊断包", null, "导出", this::exportBundle).help("保存到默认文件夹，每次生成新文件")
            .setEnabled(!busy && snapshot != null);
        data.button("分享诊断包", null, "分享", this::shareBundle).help("通过其他应用发送一份诊断包")
            .setEnabled(!busy && snapshot != null);
        data.button("重置所有设置", "恢复为出厂设置，词库不受影响", "重置", this::confirmReset)
            .setEnabled(!busy && snapshot != null);
    }

    private void uploadToggle(GroupCard group, String title, String help, AndroidLocalSettings.Snapshot settings,
            String key) {
        group.toggle(title, null, settings.bool(key), on -> edit(key, on)).help(help);
    }

    private void renderUploaded(LinearLayout target, DiagnosticsApi.Snapshot uploaded) {
        GroupCard group = GroupCard.add(sectionColumn(target, "日志共享"), "日志快照 · 存于水杉云");
        group.button("上传时间", when(uploaded.createdAt()) + " · 共 " + size(uploaded.bytes()) + " · 之后产生的日志需重新上传",
            "重新上传", () -> {
                confirming = true;
                render();
                scrollView().smoothScrollTo(0, 0);
            }).setEnabled(!busy);
        String url = uploaded.mcpUrl();
        group.button("远程地址", url, "复制", () -> copy("远程地址", url, "已复制远程地址"));
        String hint = uploaded.tokenHint();
        group.button("访问令牌", "msk_••••••••" + hint, "重新生成", this::confirmRegenerate).setEnabled(!busy);
        group.button("发送给开发者", null, "分享",
            () -> shareConnection(url)).help("地址和令牌一起打包，粘贴到开发者的 MCP 客户端即可连接").setEnabled(!busy);
    }

    private void renderAccesses(LinearLayout target) {
        GroupCard group = GroupCard.add(sectionColumn(target, "日志共享"), "最近访问");
        List<DiagnosticsApi.Access> accesses = cloud.accesses();
        if (accesses.isEmpty()) {
            group.note("开发者还没有读取过这份快照");
            return;
        }
        for (int i = 0; i < accesses.size(); i++) {
            DiagnosticsApi.Access access = accesses.get(i);
            int index = i;
            boolean open = expanded.contains(index);
            group.button(access.tool(), accessSummary(access, open), open ? "收起" : "详情", () -> {
                if (!expanded.remove(index)) expanded.add(index);
                render();
            });
        }
    }

    // ---- 操作 ----

    private void upload() {
        DiagnosticsApi.Include include = include(local);
        DiagnosticsApi.Retention retention = DiagnosticsApi.Retention.fromWire(
            local.choice(AndroidLocalSettings.MCP_RETENTION));
        if (!include.any()) return;
        busy = true;
        long request = ++generation;
        render();
        String expectedAccountId = cloudAccountId;
        String expectedSessionId = cloudSessionId;
        HostTask.runNetwork(this, context -> {
            try {
                JSONObject bundle = bundleRequest(context, include);
                if (bundle == null) return new UploadResult(null, null, "诊断包生成失败，请重试");
                requirePage(context, expectedAccountId);
                DiagnosticsApi.Created created = DiagnosticsApi.upload(context, PLATFORM,
                    AppVersionPolicy.versionName(context, ""), bundle, retention);
                DiagnosticsApi.State state;
                try {
                    state = DiagnosticsApi.state(context, created.userId());
                } catch (AccountStore.Failure failure) {
                    state = null;
                }
                return new UploadResult(created, state, null);
            } catch (AccountStore.Failure failure) {
                return new UploadResult(null, null, failureMessage(failure));
            }
        }, result -> {
            if (request != generation) {
                busy = false;
                return;
            }
            busy = false;
            if (result == null || result.created == null) {
                MsToast.show(requireContext(), result == null ? "上传失败，请重试" : result.error);
                render();
                return;
            }
            confirming = false;
            freshToken = result.created.token();
            // 之后的查看、换令牌和删除都绑定到这次上传所用的账号。
            cloudSessionId = result.created.userId();
            expanded.clear();
            if (result.state != null) {
                cloud = result.state;
                cloudLoaded = true;
            }
            render();
            MsToast.show(requireContext(), "已上传，开发者现在可以读取");
            if (result.state == null) reload();
        });
    }

    private record UploadResult(@Nullable DiagnosticsApi.Created created, @Nullable DiagnosticsApi.State state,
            @Nullable String error) {}

    private void confirmDelete() {
        String expectedAccountId = cloudAccountId;
        String expectedSessionId = cloudSessionId;
        ScreenCaptureProtection.show(new MaterialAlertDialogBuilder(requireContext())
            .setTitle("删除云端日志")
            .setMessage("云端快照和访问令牌会立即删除，开发者无法再读取。")
            .setNegativeButton("取消", (dialog, which) -> render())
            .setOnCancelListener(dialog -> render())
            .setPositiveButton("删除", (dialog, which) -> {
                busy = true;
                long request = ++generation;
                render();
                HostTask.runNetwork(this, context -> {
                    try {
                        requirePage(context, expectedAccountId);
                        DiagnosticsApi.delete(context, expectedSessionId);
                        return "";
                    } catch (AccountStore.Failure failure) {
                        return failureMessage(failure);
                    }
                }, error -> {
                    if (request != generation) {
                        busy = false;
                        return;
                    }
                    busy = false;
                    if (error == null || !error.isEmpty()) {
                        MsToast.show(requireContext(), error == null ? "删除失败，请重试" : error);
                    } else {
                        cloud = DiagnosticsApi.State.EMPTY;
                        freshToken = null;
                        expanded.clear();
                        MsToast.show(requireContext(), "已删除云端日志");
                    }
                    render();
                });
            })
            .create());
    }

    private void confirmRegenerate() {
        MessageDialog.confirm(requireContext(), "重新生成访问令牌",
            "旧令牌会立即失效，已经拿到它的开发者需要换用新令牌。", "重新生成",
            () -> regenerate(token -> MsToast.show(requireContext(), "已生成新令牌，可用「发送给开发者」分享")));
    }

    private void regenerate(Consumer<String> then) {
        busy = true;
        long request = ++generation;
        render();
        String expectedAccountId = cloudAccountId;
        String expectedSessionId = cloudSessionId;
        HostTask.runNetwork(this, context -> {
            try {
                requirePage(context, expectedAccountId);
                String token = DiagnosticsApi.regenerateToken(context, expectedSessionId);
                return new String[] {token, null};
            } catch (AccountStore.Failure failure) {
                return new String[] {null, failureMessage(failure)};
            }
        }, result -> {
            if (request != generation) {
                busy = false;
                return;
            }
            busy = false;
            if (result == null || result[0] == null) {
                MsToast.show(requireContext(), result == null ? "重新生成失败，请重试" : result[1]);
                render();
                return;
            }
            freshToken = result[0];
            render();
            reload();
            then.accept(result[0]);
        });
    }

    private void shareConnection(String url) {
        String token = freshToken;
        if (token != null) {
            share(url, token);
            return;
        }
        // 云端只存令牌的哈希，完整令牌只在生成时出现一次；离开过本页就只能换一枚新的再发。
        MessageDialog.confirm(requireContext(), "发送给开发者",
            "完整令牌只在生成时显示一次。要生成一枚新令牌并分享吗？旧令牌会立即失效。",
            "生成并分享", () -> regenerate(fresh -> share(url, fresh)));
    }

    private void share(String url, String token) {
        String text = "水杉 MCP 日志快照\n地址：" + url + "\n令牌：" + token;
        Intent send = new Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, text);
        startActivity(Intent.createChooser(send, "只发给你信任的开发者"));
        MsToast.show(requireContext(), "令牌可读取这份快照，只发给开发者");
    }

    private void copy(String label, String text, String done) {
        ClipboardActions.copyText(requireContext(), label, text, done);
    }

    private void exportBundle() {
        if (busy) return;
        if (local.text(AndroidLocalSettings.DIAGNOSTICS_EXPORT_TREE).isEmpty()) {
            pickExportDirectory(true);
            return;
        }
        busy = true;
        render();
        HostTask.run(this, DeveloperPage::exportToDirectory, message -> {
            busy = false;
            render();
            MsToast.show(requireContext(), message == null ? "诊断包导出失败，请重试" : message);
        });
    }

    private void pickExportDirectory(boolean thenExport) {
        if (busy) return;
        exportAfterPicking = thenExport;
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).addFlags(
            Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION | Intent.FLAG_GRANT_PREFIX_URI_PERMISSION);
        String previous = local.text(AndroidLocalSettings.DIAGNOSTICS_EXPORT_TREE);
        if (!previous.isEmpty()) intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, Uri.parse(previous));
        try {
            chooseExportDirectory.launch(intent);
        } catch (ActivityNotFoundException unavailable) {
            exportAfterPicking = false;
            MsToast.show(requireContext(), "系统文件夹选择器不可用");
        }
    }

    private void onExportDirectoryPicked(ActivityResult result) {
        boolean thenExport = exportAfterPicking;
        exportAfterPicking = false;
        Intent data = result.getData();
        if (result.getResultCode() != Activity.RESULT_OK || data == null || data.getData() == null
                || getView() == null || busy) return;
        Uri selected = data.getData();
        int flags = data.getFlags();
        busy = true;
        render();
        HostTask.run(this, context -> {
            try {
                DiagnosticsExportDirectory.remember(context, selected, flags);
                return thenExport ? exportToDirectory(context) : "已设置默认导出文件夹";
            } catch (IOException | RuntimeException failed) {
                return "无法保存该文件夹，请选择可读写的文件夹并允许长期访问";
            }
        }, message -> {
            busy = false;
            reload();
            MsToast.show(requireContext(), message == null ? "默认导出文件夹设置失败" : message);
        });
    }

    private static String exportDirectoryName(Context context) {
        String selected = DiagnosticsExportDirectory.saved(context);
        if (selected.isEmpty()) return "未选择";
        try {
            return DiagnosticsExportDirectory.inspect(context, selected).name();
        } catch (IOException | RuntimeException unavailable) {
            return "目录不可用，请重新选择";
        }
    }

    private static String exportToDirectory(Context context) {
        String selected = DiagnosticsExportDirectory.saved(context);
        try {
            DiagnosticsExportDirectory.inspect(context, selected);
        } catch (IOException | RuntimeException unavailable) {
            return "导出文件夹不可用，请重新选择默认导出文件夹";
        }
        File archive = null;
        try {
            AndroidLocalSettings.Snapshot settings = AndroidLocalSettings.load(context);
            DiagnosticsApi.Include include = new DiagnosticsApi.Include(true, true,
                settings.bool(AndroidLocalSettings.MCP_INPUT_EVENTS), true);
            String path = createExportBundle(context, include);
            if (path == null) return "诊断包生成失败，请重试";
            archive = new File(path);
            String directory = DiagnosticsExportDirectory.write(context, selected, archive);
            return "已导出到「" + directory + "」：" + archive.getName();
        } catch (IOException | RuntimeException failed) {
            return "诊断包未导出成功，请检查文件夹权限和剩余空间；若有未完成文件，请手动删除";
        } finally {
            if (archive != null) FilePolicy.deleteQuietly(archive);
        }
    }

    private void shareBundle() {
        if (busy) return;
        DiagnosticsApi.Include include = new DiagnosticsApi.Include(true, true,
            local.bool(AndroidLocalSettings.MCP_INPUT_EVENTS), true);
        busy = true;
        render();
        HostTask.run(this, context -> createExportBundle(context, include), path -> {
            busy = false;
            render();
            if (path == null) {
                MsToast.show(requireContext(), "诊断包生成失败，请重试");
                return;
            }
            Context context = requireContext();
            Uri uri = FileProvider.getUriForFile(context, context.getPackageName() + ".files", new File(path));
            Intent send = new Intent(Intent.ACTION_SEND).setType("application/zip")
                .putExtra(Intent.EXTRA_STREAM, uri).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            startActivity(Intent.createChooser(send, "分享诊断包"));
        });
    }

    @Nullable private static String createExportBundle(Context context, DiagnosticsApi.Include include) {
        File directory = diagnosticsCache(context);
        String name = "msime-diagnostics-" + DateTimeFormatter.ofPattern("yyyyMMdd-HHmmss", Locale.ROOT)
            .format(ZonedDateTime.now()) + "-" + UUID.randomUUID() + ".zip";
        long expired = System.currentTimeMillis() - 24L * 60 * 60 * 1000;
        File[] old = directory.listFiles((dir, file) -> file.startsWith("msime-diagnostics-") && file.endsWith(".zip"));
        if (old != null) {
            for (File stale : old) {
                if (stale.isFile() && stale.lastModified() < expired) FilePolicy.deleteQuietly(stale);
            }
        }
        File destination = new File(directory, name);
        String path = writeBundle(context, include, destination);
        if (path == null) FilePolicy.deleteQuietly(destination);
        return path;
    }

    private void confirmReset() {
        MessageDialog.confirm(requireContext(), "重置所有设置",
            "所有设置恢复为出厂设置，词库、统计和服务凭据不受影响。", "重置", () -> {
                if (SettingsResetAuthenticationPolicy.requiresAuthentication()) {
                    requestResetAuthentication();
                    return;
                }
                resetSettings();
            });
    }

    private void requestResetAuthentication() {
        int authenticators = SystemAuthenticators.allowed();
        Context context = requireContext();
        if (BiometricManager.from(context).canAuthenticate(authenticators)
                != BiometricManager.BIOMETRIC_SUCCESS) {
            MsToast.show(context, "这台设备没有可用的系统认证");
            return;
        }
        resetAuthentication = new BiometricPrompt(this, ContextCompat.getMainExecutor(context),
            new BiometricPrompt.AuthenticationCallback() {
                @Override public void onAuthenticationSucceeded(BiometricPrompt.AuthenticationResult result) {
                    resetAuthentication = null;
                    if (getView() != null) resetSettings();
                }

                @Override public void onAuthenticationError(int errorCode, CharSequence errorString) {
                    resetAuthentication = null;
                    if (getContext() != null) MsToast.show(requireContext(), "未通过系统认证，没有重置设置");
                }
            });
        BiometricPrompt.PromptInfo prompt = new BiometricPrompt.PromptInfo.Builder()
            .setTitle("验证身份")
            .setSubtitle("重置所有设置会恢复为出厂值")
            .setAllowedAuthenticators(authenticators)
            .build();
        resetAuthentication.authenticate(prompt);
    }

    private void resetSettings() {
                busy = true;
                render();
                HostTask.run(this, context -> {
                    String directory = HostStore.directory(context);
                    JSONObject current = HostStore.loadPreferences(context);
                    if (directory.isEmpty() || current == null) return null;
                    long revision = PreferencesRevisionPolicy.read(current.opt("revision"), -1);
                    if (revision < 0) return null;
                    JSONObject restored = envelopeValue(NativeClient.restoreDefaultPreferences(directory, revision));
                    if (restored == null) return null;
                    try {
                        AndroidLocalSettings.restoreDefaults(context);
                    } catch (java.io.IOException error) {
                        return null;
                    }
                    // 重置绕过了 HostStore.savePreferences，要自己标脏，否则下一轮同步会用云端设置覆盖回去。
                    SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
                    return restored;
                }, restored -> {
                    busy = false;
                    if (restored == null) {
                        MsToast.show(requireContext(), "重置失败，请重试");
                        render();
                        reload();
                        return;
                    }
                    snapshot = restored.has("preferences") ? restored : snapshot;
                    // 重置经过了系统认证，「防止截屏」随其他本地设置回到关闭；本页所在的窗口当场跟上。
                    ScreenCaptureProtection.apply(requireActivity());
                    render();
                    reload();
                    MsToast.show(requireContext(), "已恢复出厂设置");
                });
    }

    // ---- 本地设置 ----

    /** 在工作线程上写一项本地设置；界面先按新值画，写不进去时提示并按文件里的值重画。 */
    private void edit(String key, Object value) {
        HostTask.run(this, context -> {
            try {
                return AndroidLocalSettings.put(context, key, value);
            } catch (java.io.IOException | IllegalArgumentException error) {
                return null;
            }
        }, saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "保存失败，请重试");
                reload();
                return;
            }
            local = saved;
            render();
        });
    }

    private static DiagnosticsApi.Include include(AndroidLocalSettings.Snapshot settings) {
        return new DiagnosticsApi.Include(settings.bool(AndroidLocalSettings.MCP_CRASH_LOGS),
            settings.bool(AndroidLocalSettings.MCP_PERFORMANCE_LOGS),
            settings.bool(AndroidLocalSettings.MCP_INPUT_EVENTS),
            settings.bool(AndroidLocalSettings.MCP_CONFIG_SNAPSHOT));
    }

    private void showRetentionSheet(DiagnosticsApi.Retention current) {
        OptionSheet sheet = new OptionSheet(requireContext(), "保留时长", "到期后云端快照自动删除");
        for (DiagnosticsApi.Retention retention : DiagnosticsApi.Retention.values()) {
            sheet.option(retention.label(), retention == current,
                () -> edit(AndroidLocalSettings.MCP_RETENTION, retention.wire()));
        }
        sheet.show();
    }

    private static final String[][] LEVELS = {
        {"error", "错误"}, {"warn", "警告"}, {"info", "信息"}, {"debug", "调试"},
    };

    private void showLevelSheet(String current) {
        OptionSheet sheet = new OptionSheet(requireContext(), "日志级别",
            "从上到下越来越详细，每一级都包含上面各级的内容。只影响连电脑用 adb logcat 看到的系统日志，导出的诊断包不受影响");
        for (String[] level : LEVELS) {
            sheet.option(level[1], level[0].equals(current),
                () -> edit(AndroidLocalSettings.DEVELOPER_LOG_LEVEL, level[0]));
        }
        sheet.show();
    }

    private static String levelLabel(String wire) {
        for (String[] level : LEVELS) {
            if (level[0].equals(wire)) return level[1];
        }
        return "警告";
    }

    // ---- 诊断包 ----

    private static File diagnosticsCache(Context context) {
        File directory = new File(context.getCacheDir(), DIAGNOSTICS_CACHE);
        if (!directory.isDirectory() && !directory.mkdirs()) throw new IllegalStateException("diagnostics cache");
        return directory;
    }

    /**
     * 诊断包的读取范围 `{state_root, include, sources}`：导出诊断包时再加上 destination 交给 `NativeClient.diagnosticBundle`，MCP 上传时原样交给 Rust 的 diagnostics_upload。源文件是键盘写的 `<files>/diagnostics/*.jsonl` 和 Telemetry 的崩溃记录目录，不存在的传 null。状态目录还没准备好时为 null。
     */
    @Nullable private static JSONObject bundleRequest(Context context, DiagnosticsApi.Include include) {
        String stateRoot = HostStore.directory(context);
        if (stateRoot.isEmpty()) return null;
        File files = context.getFilesDir();
        File events = new File(files, "diagnostics/input-events.jsonl");
        File perf = new File(files, "diagnostics/perf.jsonl");
        File crashes = new File(files, "telemetry/telemetry-crashes");
        try {
            return new JSONObject()
                .put("state_root", stateRoot)
                .put("include", new JSONObject()
                    .put("crash_logs", include.crashLogs())
                    .put("performance_logs", include.performanceLogs())
                    .put("input_events", include.inputEvents())
                    .put("config_snapshot", include.configSnapshot()))
                .put("sources", new JSONObject()
                    .put("crash_logs", crashes.isDirectory() ? crashes.getAbsolutePath() : JSONObject.NULL)
                    .put("performance_logs", perf.isFile() ? perf.getAbsolutePath() : JSONObject.NULL)
                    .put("input_events", events.isFile() ? events.getAbsolutePath() : JSONObject.NULL));
        } catch (JSONException malformed) {
            return null;
        }
    }

    /** 用 Rust 写诊断包（{@link #bundleRequest} 加上 destination）。成功时返回 zip 的路径。 */
    @Nullable private static String writeBundle(Context context, DiagnosticsApi.Include include, File destination) {
        JSONObject request = bundleRequest(context, include);
        if (request == null) return null;
        try {
            request.put("destination", destination.getAbsolutePath());
            JSONObject value = envelopeValue(NativeClient.diagnosticBundle(request.toString()));
            if (value == null) return null;
            String path = value.optString("path", destination.getAbsolutePath());
            return new File(path).isFile() ? path : null;
        } catch (JSONException malformed) {
            return null;
        }
    }

    @Nullable private static JSONObject envelopeValue(@Nullable String response) {
        if (response == null) return null;
        try {
            JSONObject root = new JSONObject(response);
            return JsonPolicy.objectValue(root);
        } catch (JSONException malformed) {
            return null;
        }
    }

    // ---- 文案 ----

    private static String failureMessage(AccountStore.Failure failure) {
        return AccountFailureText.diagnostics(failure.code, failure.retryAfterSeconds);
    }

    private static String categories(DiagnosticsApi.Include include) {
        List<String> names = new ArrayList<>(4);
        if (include.crashLogs()) names.add("崩溃日志");
        if (include.performanceLogs()) names.add("性能日志");
        if (include.inputEvents()) names.add("输入事件");
        if (include.configSnapshot()) names.add("配置快照");
        return String.join("、", names);
    }

    private static String accessSummary(DiagnosticsApi.Access access, boolean open) {
        String unit = "get_config_snapshot".equals(access.tool()) ? "份" : "条";
        StringBuilder text = new StringBuilder(when(access.at())).append(" · 返回 ").append(access.resultCount())
            .append(' ').append(unit);
        if (open) {
            String arguments = arguments(access.arguments());
            if (!arguments.isEmpty()) text.append(" · ").append(arguments);
            text.append(" · 共 ").append(size(access.bytes()));
        }
        return text.toString();
    }

    /** 调用参数 `{"since":"24h","level":"error"}` 显示成 `since=24h · level=error`；读不成对象时原样显示。 */
    private static String arguments(String raw) {
        if (raw == null || raw.isEmpty() || "{}".equals(raw)) return "";
        try {
            JSONObject object = new JSONObject(raw);
            List<String> parts = new ArrayList<>(object.length());
            for (Iterator<String> keys = object.keys(); keys.hasNext(); ) {
                String key = keys.next();
                parts.add(key + "=" + object.opt(key));
            }
            return String.join(" · ", parts);
        } catch (JSONException notObject) {
            return raw;
        }
    }

    private static String size(long bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return Math.round(bytes / 1024.0) + " KB";
        return NumberPolicy.decimal1(bytes / (1024.0 * 1024.0)) + " MB";
    }

    /** ISO 时间显示成「今天 14:28」或「10月4日 14:28」。 */
    private static String when(String iso) {
        ZonedDateTime time = parse(iso);
        if (time == null) return "";
        if (time.toLocalDate().equals(LocalDate.now(ZoneId.systemDefault()))) return "今天 " + CLOCK.format(time);
        return DAY_CLOCK.format(time);
    }

    private static String remaining(String iso) {
        ZonedDateTime expires = parse(iso);
        if (expires == null) return "到期后自动删除";
        long minutes = java.time.Duration.between(Instant.now(), expires.toInstant()).toMinutes();
        if (minutes <= 0) return "即将自动删除";
        if (minutes < 60) return minutes + " 分钟后自动删除";
        long hours = (minutes + 59) / 60;
        if (hours <= 48) return hours + " 小时后自动删除";
        return ((hours + 23) / 24) + " 天后自动删除";
    }

    @Nullable private static ZonedDateTime parse(String iso) {
        if (iso == null || iso.isEmpty()) return null;
        return TimestampDisplayPolicy.parse(iso);
    }
}
