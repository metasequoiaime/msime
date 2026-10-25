package app.msime.android.home;

import android.content.Context;
import android.widget.LinearLayout;
import androidx.biometric.BiometricManager;
import androidx.biometric.BiometricPrompt;
import androidx.core.content.ContextCompat;
import androidx.annotation.Nullable;
import app.msime.android.InputFeatureToggle;
import app.msime.android.ClipboardHistoryAuthenticationPolicy;
import app.msime.android.SystemAuthenticators;
import app.msime.android.R;
import app.msime.android.core.Telemetry;
import java.util.EnumMap;
import java.util.Map;
import org.json.JSONObject;

/**
 * 隐私：联网功能（用水杉账号翻译候选、匿名使用统计，以及去「翻译服务」的入口）、本机数据（剪贴板历史）、隐私模式说明和隐私政策链接。
 *
 * <p>三个开关就是 {@link InputFeatureToggle} 里的那三项，标题和说明也取那里，本页是它们在宿主里唯一的位置（实施计划 §2.9）。隐私模式的语义在 Rust 与键盘里（P17）：这里只说明它做什么、不做什么。
 */
public final class PrivacyPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("联网", "本机", "无痕");
    }

    private static final String POLICY = "https://msime.app/privacy/";
    private static final InputFeatureToggle[] NETWORK = {
        InputFeatureToggle.CANDIDATE_TRANSLATION_ACCOUNT, InputFeatureToggle.USAGE_REPORTING,
    };
    private static final InputFeatureToggle[] LOCAL = {InputFeatureToggle.CLIPBOARD_HISTORY};

    private final Map<InputFeatureToggle, GroupCard.Row> rows = new EnumMap<>(InputFeatureToggle.class);
    @Nullable private JSONObject preferences;
    @Nullable private Boolean pendingClipboardHistoryValue;
    @Nullable private BiometricPrompt clipboardAuthentication;

    @Override protected void onContentReady() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        rows.clear();
        String app = getString(R.string.app_name);
        GroupCard intro = GroupCard.add(sectionColumn(target, "联网"), null);
        intro.note(app + "本地优先：拼音、词库、自造词和输入统计都在这台设备上处理和保存。只有你打开的联网功能才会连接 api.msime.app，下面逐项列出。");

        GroupCard network = GroupCard.add(sectionColumn(target, "联网"), "联网功能").withDividers(16);
        for (InputFeatureToggle toggle : NETWORK) addToggle(network, toggle);
        network.nav("翻译服务", "改用小牛、腾讯或自定义（DeepLX、DeepL）服务", null,
            () -> SettingsNavigator.open(requireContext(), PageId.TRANSLATION_SERVICE, null));
        network.note("在「翻译服务」里选了小牛、腾讯或自定义服务时，键盘会把当前一页的中文候选词直接发给你配置的第三方服务翻译，不经过水杉服务器，按该服务自己的数据政策处理；凭据只保存在本机。");

        GroupCard local = GroupCard.add(sectionColumn(target, "本机"), "本机数据").withDividers(16);
        for (InputFeatureToggle toggle : LOCAL) addToggle(local, toggle);

        GroupCard incognito = GroupCard.add(sectionColumn(target, "无痕"), "隐私模式");
        incognito.note("在键盘功能面板里打开隐私模式后，" + app
            + "不学习新词、不调整候选顺序，不记录字数、按键和输入统计，不保存剪贴板历史和云剪贴板，也不做任何数据贡献。云候选、翻译、AI 润色这类你单独打开的联网功能按各自的开关照常工作，不受隐私模式影响。");

        GroupCard links = GroupCard.add(sectionColumn(target, "无痕"), null);
        links.nav("隐私政策", "msime.app", null, this::openPolicy);
        render();
    }

    @Override protected boolean shouldReloadOnContentReady() { return false; }

    @Override protected void onContentDestroyed() {
        if (clipboardAuthentication != null) clipboardAuthentication.cancelAuthentication();
        clipboardAuthentication = null;
        pendingClipboardHistoryValue = null;
        rows.clear();
    }

    private void addToggle(GroupCard group, InputFeatureToggle toggle) {
        GroupCard.Row row = group.toggle(toggle.title(), toggle.description(), toggle.enabledByDefault(),
            value -> save(toggle, value));
        row.setEnabled(false);
        rows.put(toggle, row);
    }

    @Override protected void reload() {
        HostTask.run(this, HostStore::loadPreferences, snapshot -> {
            preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
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
     * 写一个开关。打开「用水杉账号翻译候选」等于在「翻译服务」里选水杉账号：同一次读改写里关掉小牛、自定义和腾讯，否则用户自己的服务仍然优先，这个开关开了也不生效。与共享设置页的 `setTranslationProvider("account")` 相同；关掉它只删 `translation_account`，不动其他服务。
     */
    @Nullable private static JSONObject write(Context context, InputFeatureToggle toggle, boolean value) {
        if (toggle != InputFeatureToggle.CANDIDATE_TRANSLATION_ACCOUNT || !value)
            return HostStore.putPreference(context, toggle.key(), value);
        return KeyboardSheets.write(context, preferences -> {
            KeyboardSheets.child(preferences, "niutrans").put("enabled", false);
            KeyboardSheets.child(preferences, "custom_translation").put("enabled", false);
            KeyboardSheets.child(preferences, "tencent_tmt").put("enabled", false);
            preferences.put(toggle.key(), true);
        });
    }

    private void openPolicy() {
        if (!LinkActions.openBrowsable(requireContext(), POLICY)) {
            MsToast.show(requireContext(), "这台设备上没有可以打开网页的应用");
        }
    }
}
