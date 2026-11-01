package app.msime.android.home;

import android.app.Activity;
import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.widget.LinearLayout;
import androidx.biometric.BiometricManager;
import androidx.biometric.BiometricPrompt;
import androidx.core.content.ContextCompat;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.CommunityCatalog;
import app.msime.android.InputFeatureToggle;
import app.msime.android.ClipboardHistoryAuthenticationPolicy;
import app.msime.android.ScreenCaptureProtection;
import app.msime.android.ScreenCaptureProtectionPolicy;
import app.msime.android.SystemAuthenticators;
import app.msime.android.R;
import app.msime.android.core.Telemetry;
import java.util.EnumMap;
import java.util.LinkedHashMap;
import java.util.Map;
import org.json.JSONObject;

/**
 * 隐私：联网功能（用水杉账号翻译候选、匿名使用统计，以及去「翻译服务」的入口）、本机数据（剪贴板历史）、防止截屏、隐私模式说明和隐私政策链接。
 *
 * <p>三个开关就是 {@link InputFeatureToggle} 里的那三项，标题和说明也取那里，本页是它们在宿主里唯一的位置（实施计划 §2.9）。隐私模式的语义在 Rust 与键盘里（P17）：这里只说明它做什么、不做什么。
 *
 * <p>「防止截屏」（#10098）是 Android 本地设置，不在共享偏好里；切换规则见 {@link ScreenCaptureProtectionPolicy}，窗口上的效果见 {@link ScreenCaptureProtection}。
 *
 * <p>「社区」组的「已屏蔽的作者」进 {@link BlockedAuthorsPage}。屏蔽只认登录的水杉账号，本页不登录也能打开，所以没有登录时这一行写「登录后可用」，点了先请人登录（{@link CommunityBlockDialog#requireSignIn}）。
 */
public final class PrivacyPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("联网", "本机", "无痕");
    }

    private static final String POLICY = "https://msime.app/privacy/";
    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static final InputFeatureToggle[] NETWORK = {
        InputFeatureToggle.CANDIDATE_TRANSLATION_ACCOUNT, InputFeatureToggle.USAGE_REPORTING,
    };
    private static final InputFeatureToggle[] LOCAL = {InputFeatureToggle.CLIPBOARD_HISTORY};

    private final Map<InputFeatureToggle, GroupCard.Row> rows = new EnumMap<>(InputFeatureToggle.class);
    @Nullable private JSONObject preferences;
    @Nullable private Boolean pendingClipboardHistoryValue;
    @Nullable private BiometricPrompt clipboardAuthentication;
    @Nullable private GroupCard.Row screenCaptureRow;
    /** 读到的本地设置；还没读到时开关保持禁用。 */
    @Nullable private AndroidLocalSettings.Snapshot localSettings;
    /** 正在认证或正在写入的目标值；非空时开关禁用，避免重复切换。 */
    @Nullable private Boolean pendingScreenCaptureValue;
    @Nullable private BiometricPrompt screenCaptureAuthentication;
    @Nullable private GroupCard.Row blockedAuthorsRow;
    /** 有没有登录水杉账号；还没读到时为 null，「已屏蔽的作者」那一行先不写副标题。 */
    @Nullable private Boolean signedIn;

    @Override protected void onContentReady() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        rows.clear();
        String app = getString(R.string.app_name);
        // 页首那段「本地优先」说明的是下面这组联网功能，收进这组标题的「?」，不再单独占一张卡片。
        GroupCard network = GroupCard.add(sectionColumn(target, "联网"), "联网功能").withDividers(16);
        network.help(app + "本地优先：拼音、词库、自造词和输入统计都在这台设备上处理和保存。只有你打开的联网功能才会连接 api.msime.app，下面逐项列出。");
        for (InputFeatureToggle toggle : NETWORK) addToggle(network, toggle);
        network.nav("翻译服务", null, null,
            () -> SettingsNavigator.open(requireContext(), PageId.TRANSLATION_SERVICE, null)).help("改用小牛、腾讯或自定义（DeepLX、DeepL）服务");
        network.help("在「翻译服务」里选了小牛、腾讯或自定义服务时，键盘会把当前一页的中文候选词直接发给你配置的第三方服务翻译，不经过水杉服务器，按该服务自己的数据政策处理；凭据只保存在本机。");

        GroupCard community = GroupCard.add(sectionColumn(target, "联网"), "社区");
        blockedAuthorsRow = community.nav("已屏蔽的作者", null, null, () -> CommunityBlockDialog.requireSignIn(this,
            () -> SettingsNavigator.open(requireContext(), PageId.BLOCKED_AUTHORS, null)));

        GroupCard local = GroupCard.add(sectionColumn(target, "本机"), "本机数据").withDividers(16);
        for (InputFeatureToggle toggle : LOCAL) addToggle(local, toggle);

        GroupCard capture = GroupCard.add(sectionColumn(target, "本机"), "截屏与录屏");
        screenCaptureRow = capture.toggle("防止截屏", null, false,
            this::saveScreenCaptureProtection).help("设置页面和键盘在截屏、录屏里显示为黑色");
        screenCaptureRow.setEnabled(false);
        capture.help("打开后，" + app + "设置里的页面和在其他应用里弹出的键盘不能被截屏、录屏或投屏，截出来是黑的，最近任务里也看不到设置页的内容。"
            + "第一次打开不需要验证；打开过以后，再打开或关闭都要通过锁屏密码或生物识别验证。在系统设置里清除" + app + "的数据会让它回到关闭。");

        GroupCard incognito = GroupCard.add(sectionColumn(target, "无痕"), "隐私模式");
        incognito.note("在键盘功能面板里打开隐私模式后，" + app
            + "不学习新词、不调整候选顺序，不记录字数、按键和输入统计，不保存剪贴板历史和云剪贴板，也不做任何数据贡献。云候选、翻译、AI 润色这类你单独打开的联网功能按各自的开关照常工作，不受隐私模式影响。");

        GroupCard links = GroupCard.add(sectionColumn(target, "无痕"), null);
        links.nav("隐私政策", null, null, this::openPolicy).help("msime.app");
        render();
    }

    @Override protected boolean shouldReloadOnContentReady() { return false; }

    @Override protected void onContentDestroyed() {
        if (clipboardAuthentication != null) clipboardAuthentication.cancelAuthentication();
        clipboardAuthentication = null;
        pendingClipboardHistoryValue = null;
        if (screenCaptureAuthentication != null) screenCaptureAuthentication.cancelAuthentication();
        screenCaptureAuthentication = null;
        pendingScreenCaptureValue = null;
        screenCaptureRow = null;
        blockedAuthorsRow = null;
        rows.clear();
    }

    private void addToggle(GroupCard group, InputFeatureToggle toggle) {
        GroupCard.Row row = group.toggle(toggle.title(), null, toggle.enabledByDefault(),
            value -> save(toggle, value)).help(toggle.description());
        row.setEnabled(false);
        rows.put(toggle, row);
    }

    @Override protected void reload() {
        HostTask.run(this, HostStore::loadPreferences, snapshot -> {
            preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
            render();
        });
        HostTask.run(this, AndroidLocalSettings::load, snapshot -> {
            localSettings = snapshot;
            render();
        });
        // 读登录状态要等 Rust 的会话锁，放在网络线程上；登录或退出后回到本页时重新读。
        HostTask.runNetwork(this, context -> new CommunityCatalog(context).signedIn(), value -> {
            signedIn = value;
            render();
        });
    }

    private void render() {
        for (Map.Entry<InputFeatureToggle, GroupCard.Row> entry : rows.entrySet()) {
            InputFeatureToggle toggle = entry.getKey();
            GroupCard.Row row = entry.getValue();
            // 偏好还没读到时开关保持禁用，避免用户在默认值上切换后被读到的真实值覆盖。
            row.setEnabled(preferences != null);
            if (preferences != null) row.setChecked(preferences.optBoolean(toggle.key(), toggle.enabledByDefault()));
        }
        if (blockedAuthorsRow != null) blockedAuthorsRow.setSubtitle(Boolean.FALSE.equals(signedIn) ? "登录后可用" : null);
        if (screenCaptureRow != null) {
            screenCaptureRow.setEnabled(localSettings != null && pendingScreenCaptureValue == null);
            if (localSettings != null) screenCaptureRow.setChecked(localSettings.bool(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION));
        }
    }

    /** 切换「防止截屏」：第一次打开直接保存，打开过以后先认证；设备没有系统认证时一律不切换，否则打开以后再也关不掉。 */
    private void saveScreenCaptureProtection(boolean value) {
        AndroidLocalSettings.Snapshot current = localSettings;
        if (current == null || pendingScreenCaptureValue != null) {
            render();
            return;
        }
        int authenticators = SystemAuthenticators.allowed();
        boolean available = BiometricManager.from(requireContext()).canAuthenticate(authenticators)
            == BiometricManager.BIOMETRIC_SUCCESS;
        switch (ScreenCaptureProtectionPolicy.decide(
                current.bool(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION),
                current.bool(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION_ENABLED_BEFORE), value, available)) {
            case UNCHANGED -> render();
            case SAVE -> writeScreenCaptureProtection(value);
            case UNAVAILABLE -> {
                render();
                MsToast.show(requireContext(), value
                    ? "这台设备没有可用的系统认证，设置锁屏密码后才能打开防止截屏"
                    : "这台设备没有可用的系统认证，防止截屏保持开启");
            }
            case AUTHENTICATE -> requestScreenCaptureAuthentication(value, authenticators);
        }
    }

    private void requestScreenCaptureAuthentication(boolean value, int authenticators) {
        pendingScreenCaptureValue = value;
        render();
        screenCaptureAuthentication = new BiometricPrompt(this,
            ContextCompat.getMainExecutor(requireContext()), new BiometricPrompt.AuthenticationCallback() {
                @Override public void onAuthenticationSucceeded(BiometricPrompt.AuthenticationResult result) {
                    finishScreenCaptureAuthentication(true);
                }

                @Override public void onAuthenticationError(int errorCode, CharSequence errorString) {
                    finishScreenCaptureAuthentication(false);
                }
            });
        BiometricPrompt.PromptInfo prompt = new BiometricPrompt.PromptInfo.Builder()
            .setTitle("验证身份")
            .setSubtitle(value ? "打开防止截屏" : "关闭防止截屏")
            .setAllowedAuthenticators(authenticators)
            .build();
        screenCaptureAuthentication.authenticate(prompt);
    }

    private void finishScreenCaptureAuthentication(boolean authenticated) {
        Boolean value = pendingScreenCaptureValue;
        screenCaptureAuthentication = null;
        if (value == null) return;
        if (authenticated) {
            writeScreenCaptureProtection(value);
            return;
        }
        pendingScreenCaptureValue = null;
        render();
        Context context = getContext();
        if (context != null) MsToast.show(context, value ? "未通过系统认证，防止截屏保持关闭" : "未通过系统认证，防止截屏保持开启");
    }

    /**
     * 写入开关；打开时同一次写入记下「打开过」，此后的切换都要认证。写好后当场给本页所在 Activity 的窗口加上或去掉保护。
     *
     * <p>窗口不跟着本页的视图走：{@link HostTask} 在页面视图销毁后会丢掉结果回调，而写入排在共用的工作线程上可能晚于用户退出本页完成。若靠那个回调改窗口，用户打开后立刻返回时文件已经写成「开着」，设置应用的其他页面却一直可以截屏，要等下次 {@code onResume} 才跟上。所以写成后直接把窗口更新投到主线程，只认 Activity 是否还在。
     */
    private void writeScreenCaptureProtection(boolean value) {
        pendingScreenCaptureValue = value;
        render();
        Activity activity = requireActivity();
        HostTask.run(this, context -> {
            Map<String, Object> edits = new LinkedHashMap<>(2);
            edits.put(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION, value);
            if (value) edits.put(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION_ENABLED_BEFORE, true);
            final AndroidLocalSettings.Snapshot saved;
            try {
                saved = AndroidLocalSettings.update(context, edits);
            } catch (java.io.IOException | IllegalArgumentException error) {
                return null;
            }
            boolean protect = saved.bool(AndroidLocalSettings.SCREEN_CAPTURE_PROTECTION);
            MAIN.post(() -> {
                if (!activity.isDestroyed()) ScreenCaptureProtection.apply(activity.getWindow(), protect);
            });
            return saved;
        }, saved -> {
            pendingScreenCaptureValue = null;
            if (saved == null) {
                MsToast.show(requireContext(), "没有保存，请重试");
            } else {
                localSettings = saved;
            }
            render();
        });
    }

    private void save(InputFeatureToggle toggle, boolean value) {
        if (toggle == InputFeatureToggle.CLIPBOARD_HISTORY && preferences != null
                && ClipboardHistoryAuthenticationPolicy.requiresAuthentication(
                    preferences.optBoolean(toggle.key(), toggle.enabledByDefault()), value)) {
            pendingClipboardHistoryValue = value;
            GroupCard.Row row = rows.get(toggle);
            if (row != null) row.setEnabled(false);
            if (!requestClipboardAuthentication()) {
                pendingClipboardHistoryValue = null;
                render();
                MsToast.show(requireContext(), "这台设备没有可用的系统认证");
            }
            return;
        }
        savePreference(toggle, value);
    }

    private boolean requestClipboardAuthentication() {
        int authenticators = SystemAuthenticators.allowed();
        BiometricManager manager = BiometricManager.from(requireContext());
        if (manager.canAuthenticate(authenticators) != BiometricManager.BIOMETRIC_SUCCESS) return false;
        clipboardAuthentication = new BiometricPrompt(this,
            ContextCompat.getMainExecutor(requireContext()), new BiometricPrompt.AuthenticationCallback() {
                @Override public void onAuthenticationSucceeded(BiometricPrompt.AuthenticationResult result) {
                    finishClipboardAuthentication(true);
                }

                @Override public void onAuthenticationError(int errorCode, CharSequence errorString) {
                    finishClipboardAuthentication(false);
                }
            });
        BiometricPrompt.PromptInfo prompt = new BiometricPrompt.PromptInfo.Builder()
            .setTitle("验证身份")
            .setSubtitle("关闭剪贴板历史会清空本机记录")
            .setAllowedAuthenticators(authenticators)
            .build();
        clipboardAuthentication.authenticate(prompt);
        return true;
    }

    private void finishClipboardAuthentication(boolean authenticated) {
        Boolean value = pendingClipboardHistoryValue;
        pendingClipboardHistoryValue = null;
        if (value == null) return;
        clipboardAuthentication = null;
        if (authenticated) {
            savePreference(InputFeatureToggle.CLIPBOARD_HISTORY, value);
        } else {
            render();
            Context context = getContext();
            if (context != null) MsToast.show(context, "未通过系统认证，剪贴板历史保持开启");
        }
    }

    private void savePreference(InputFeatureToggle toggle, boolean value) {
        HostTask.run(this, context -> write(context, toggle, value), saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "没有保存，请重试");
            } else {
                preferences = saved.optJSONObject("preferences");
                // 关闭要当场停止上报并清空待发数据，不能等下次启动才读到偏好。
                if (toggle == InputFeatureToggle.USAGE_REPORTING) Telemetry.setEnabled(requireContext(), value);
            }
            render();
        });
    }

    /**
     * 写一个开关。打开「用水杉账号翻译候选」等于在「翻译服务」里选水杉账号：同一次读改写里关掉小牛、自定义和腾讯，并取消 AI 辅助（`translation_ai`），否则用户自己的服务仍然优先，这个开关开了也不生效。与共享设置页的 `setTranslationProvider("account")` 相同；关掉它只删 `translation_account`，不动其他服务。
     */
    @Nullable private static JSONObject write(Context context, InputFeatureToggle toggle, boolean value) {
        if (toggle != InputFeatureToggle.CANDIDATE_TRANSLATION_ACCOUNT || !value)
            return HostStore.putPreference(context, toggle.key(), value);
        return KeyboardSheets.write(context, preferences -> {
            KeyboardSheets.child(preferences, "niutrans").put("enabled", false);
            KeyboardSheets.child(preferences, "custom_translation").put("enabled", false);
            KeyboardSheets.child(preferences, "tencent_tmt").put("enabled", false);
            preferences.remove("translation_ai");
            preferences.put(toggle.key(), true);
        });
    }

    private void openPolicy() {
        if (!LinkActions.openBrowsable(requireContext(), POLICY)) {
            MsToast.show(requireContext(), "这台设备上没有可以打开网页的应用");
        }
    }
}
