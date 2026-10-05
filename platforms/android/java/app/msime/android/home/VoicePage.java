package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.DoubaoAsrPolicy;
import app.msime.android.VoiceConfiguration;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 语音输入页：识别语言、自动添加标点、离线识别、启动方式，以及隐私组的「上传语音以改进识别」。
 *
 * <p>键盘读同一批存储：语言是共享偏好的 `voice_input.language`（普通话 `zh-cn`、粤语 `yue`、英语 `en`、普通话 + 英语 `auto`）；自动标点绑定已有的 `voice_input.doubao_enable_punc`，只有豆包识别支持，其他识别器下置灰并写明原因；离线识别只有 Android 有，在 {@link AndroidLocalSettings} 的 `platform.android.voice_offline_fallback`；启动方式没有自己的字段，由本地设置的长按空格（`platform.android.space_voice`）和共享偏好的 `touch_voice_shortcut` 联合派生，选择时两个一起写。
 *
 * <p>「上传语音以改进识别」（本地设置的 `platform.android.voice_contribute_audio`，只在本机、不同步）默认关闭；每次从关切到开都先弹确认框说明上传什么、保存多久，用户确认后才写（P19）。
 */
public final class VoicePage extends DetailPage {
    private static final String[] LANGUAGES = {"zh-cn", "yue", "en", "auto"};
    private static final String[] LANGUAGE_LABELS = {"普通话", "粤语", "英语", "普通话 + 英语"};
    private static final String[] TRIGGER_LABELS = {"长按空格", "工具栏按钮", "无"};
    private static final int TRIGGER_SPACE = 0;
    private static final int TRIGGER_TOOLBAR = 1;
    private static final int TRIGGER_NONE = 2;

    /** 页面渲染时读到的偏好与识别器。 */
    private record State(JSONObject preferences, AndroidLocalSettings.Snapshot local, boolean punctuationSupported) {}

    @Nullable private LinearLayout column;
    @Nullable private GroupCard.Row contributeRow;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        reload();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        contributeRow = null;
        super.onDestroyView();
    }

    private void reload() {
        HostTask.run(this, VoicePage::read, this::render);
    }

    @Nullable private static State read(Context context) {
        JSONObject snapshot = HostStore.loadPreferences(context);
        JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
        if (preferences == null) return null;
        VoiceConfiguration configuration = VoiceConfiguration.read(HostStore.directory(context), "settings");
        return new State(preferences, AndroidLocalSettings.load(context),
            DoubaoAsrPolicy.PROVIDER.equals(configuration.provider()));
    }

    private void render(@Nullable State state) {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        if (state == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        JSONObject preferences = state.preferences();
        AndroidLocalSettings.Snapshot local = state.local();
        JSONObject voice = preferences.optJSONObject("voice_input");
        if (voice == null) voice = new JSONObject();

        GroupCard recognition = GroupCard.add(target, "识别");
        String language = voice.optString("language", "");
        GroupCard.Row[] languageRow = new GroupCard.Row[1];
        languageRow[0] = recognition.nav("识别语言", null, languageLabel(language),
            () -> pickLanguage(language, languageRow[0]));

        boolean punctuation = voice.optBoolean("doubao_enable_punc", true);
        GroupCard.Row punctuationRow = recognition.toggle("自动添加标点",
            state.punctuationSupported() ? null : "当前识别服务不支持，仅豆包语音识别可以自动加标点",
            punctuation, checked -> saveVoice("doubao_enable_punc", checked));
        punctuationRow.setEnabled(state.punctuationSupported());

        recognition.toggle("离线识别", "无网络时使用本地模型，准确率略低。需要先安装本地语音模型",
            local.bool(AndroidLocalSettings.VOICE_OFFLINE_FALLBACK),
            checked -> saveLocal(AndroidLocalSettings.VOICE_OFFLINE_FALLBACK, checked));

        int trigger = trigger(local.bool(AndroidLocalSettings.SPACE_VOICE),
            preferences.optBoolean("touch_voice_shortcut", false));
        GroupCard.Row[] triggerRow = new GroupCard.Row[1];
        triggerRow[0] = recognition.nav("启动方式", null, TRIGGER_LABELS[trigger],
            () -> pickTrigger(trigger, triggerRow[0]));

        GroupCard privacy = GroupCard.add(target, "隐私");
        boolean contribute = local.bool(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO);
        contributeRow = privacy.toggle("上传语音以改进识别", "语音片段匿名处理，可随时关闭", contribute,
            this::onContributeChanged);
    }

    private void pickLanguage(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "识别语言", null);
        String current = selected.isEmpty() ? LANGUAGES[0] : selected;
        for (int index = 0; index < LANGUAGES.length; index++) {
            String value = LANGUAGES[index];
            String label = LANGUAGE_LABELS[index];
            sheet.option(label, value.equals(current), () -> {
                row.setValue(label);
                saveVoice("language", value);
            });
        }
        sheet.show();
    }

    private void pickTrigger(int selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "启动方式", null);
        for (int index = 0; index < TRIGGER_LABELS.length; index++) {
            int choice = index;
            sheet.option(TRIGGER_LABELS[index], index == selected, () -> {
                row.setValue(TRIGGER_LABELS[choice]);
                saveTrigger(choice);
            });
        }
        sheet.show();
    }

    /** 从关切到开先确认；确认前开关回到关闭，确认后才写偏好。从开切到关直接写。 */
    private void onContributeChanged(boolean checked) {
        GroupCard.Row row = contributeRow;
        if (!checked) {
            saveLocal(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO, false);
            return;
        }
        if (row != null) row.setChecked(false);
        new MaterialAlertDialogBuilder(requireContext())
            .setTitle("上传语音以改进识别？")
            .setMessage("开启后，每次语音输入的音频和识别出的文字会上传到水杉云，只用于改进语音识别。数据保存 180 天，到期自动删除；你可以随时在这里关闭。隐私模式、密码框和不允许个性化学习的输入框里不会上传。")
            .setNegativeButton("取消", null)
            .setPositiveButton("开启", (dialog, which) -> {
                GroupCard.Row current = contributeRow;
                if (current != null) current.setChecked(true);
                saveLocal(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO, true);
            })
            .show();
    }

    private void saveVoice(String member, Object value) {
        save(preferences -> {
            JSONObject voice = preferences.optJSONObject("voice_input");
            if (voice == null) {
                voice = new JSONObject();
                preferences.put("voice_input", voice);
            }
            voice.put(member, value);
        });
    }

    private void saveLocal(String key, Object value) {
        KeyboardSheets.saveLocal(this, key, value, null, this::reload);
    }

    /** 长按空格在本地设置，工具栏按钮在共享偏好；先写本地，再写共享。 */
    private void saveTrigger(int trigger) {
        HostTask.run(this, context -> KeyboardSheets.writeLocal(context, AndroidLocalSettings.SPACE_VOICE,
                trigger == TRIGGER_SPACE)
            ? write(context, preferences -> preferences.put("touch_voice_shortcut", trigger == TRIGGER_TOOLBAR))
            : null, saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "保存失败，请重试");
                reload();
            }
        });
    }

    private interface Edit {
        void apply(JSONObject preferences) throws JSONException;
    }

    private void save(Edit edit) {
        HostTask.run(this, context -> write(context, edit), saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "保存失败，请重试");
                reload();
            }
        });
    }

    @Nullable private static JSONObject write(Context context, Edit edit) {
        JSONObject snapshot = HostStore.loadPreferences(context);
        if (snapshot == null) return null;
        try {
            edit.apply(snapshot.getJSONObject("preferences"));
        } catch (JSONException error) {
            return null;
        }
        return HostStore.savePreferences(context, snapshot);
    }

    /** 启动方式由两个开关派生：长按空格优先，其次是工具栏按钮，都关时没有语音入口。 */
    private static int trigger(boolean spaceVoice, boolean voiceShortcut) {
        if (spaceVoice) return TRIGGER_SPACE;
        return voiceShortcut ? TRIGGER_TOOLBAR : TRIGGER_NONE;
    }

    /** 未设置时按普通话显示（键盘的默认识别语言）；其他平台写入的语言原样显示。 */
    private static String languageLabel(String value) {
        if (value.isEmpty()) return LANGUAGE_LABELS[0];
        for (int index = 0; index < LANGUAGES.length; index++) {
            if (LANGUAGES[index].equals(value)) return LANGUAGE_LABELS[index];
        }
        return "ja".equals(value) ? "日语" : value;
    }
}
