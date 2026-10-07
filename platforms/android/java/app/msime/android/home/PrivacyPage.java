package app.msime.android.home;

import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.InputFeatureToggle;
import app.msime.android.R;
import app.msime.android.core.Telemetry;
import java.util.EnumMap;
import java.util.Map;
import org.json.JSONObject;

/**
 * 隐私：联网功能（用水杉账号翻译候选、匿名使用统计）、本机数据（剪贴板历史）、隐私模式说明和隐私政策链接。
 *
 * <p>三个开关就是 {@link InputFeatureToggle} 里的那三项，标题和说明也取那里，本页是它们在宿主里唯一的位置（实施计划 §2.9）。隐私模式的语义在 Rust 与键盘里（P17）：这里只说明它做什么、不做什么。
 */
public final class PrivacyPage extends DetailPage {
    private static final String POLICY = "https://msime.app/privacy/";
    private static final InputFeatureToggle[] NETWORK = {
        InputFeatureToggle.CANDIDATE_TRANSLATION_ACCOUNT, InputFeatureToggle.USAGE_REPORTING,
    };
    private static final InputFeatureToggle[] LOCAL = {InputFeatureToggle.CLIPBOARD_HISTORY};

    private final Map<InputFeatureToggle, GroupCard.Row> rows = new EnumMap<>(InputFeatureToggle.class);
    @Nullable private JSONObject preferences;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        rows.clear();
        String app = getString(R.string.app_name);
        GroupCard intro = GroupCard.add(column, null);
        intro.note(app + "本地优先：拼音、词库、自造词和输入统计都在这台设备上处理和保存。只有你打开的联网功能才会连接 api.msime.app，下面逐项列出。");

        GroupCard network = GroupCard.add(column, "联网功能").withDividers(16);
        for (InputFeatureToggle toggle : NETWORK) addToggle(network, toggle);

        GroupCard local = GroupCard.add(column, "本机数据").withDividers(16);
        for (InputFeatureToggle toggle : LOCAL) addToggle(local, toggle);

        GroupCard incognito = GroupCard.add(column, "隐私模式");
        incognito.note("在键盘功能面板里打开隐私模式后，" + app
            + "不学习新词、不调整候选顺序，不记录字数、按键和输入统计，不保存剪贴板历史和云剪贴板，也不做任何数据贡献。云候选、翻译、AI 润色这类你单独打开的联网功能按各自的开关照常工作，不受隐私模式影响。");

        GroupCard links = GroupCard.add(column, null);
        links.nav("隐私政策", "msime.app", null, this::openPolicy);
        render();
    }

    @Override protected void onBecameVisible() { reload(); }

    private void addToggle(GroupCard group, InputFeatureToggle toggle) {
        GroupCard.Row row = group.toggle(toggle.title(), toggle.description(), toggle.enabledByDefault(),
            value -> save(toggle, value));
        row.setEnabled(false);
        rows.put(toggle, row);
    }

    private void reload() {
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
        HostTask.run(this, context -> HostStore.putPreference(context, toggle.key(), value), saved -> {
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

    private void openPolicy() {
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(POLICY)).addCategory(Intent.CATEGORY_BROWSABLE));
        } catch (ActivityNotFoundException absent) {
            MsToast.show(requireContext(), "这台设备上没有可以打开网页的应用");
        }
    }
}
