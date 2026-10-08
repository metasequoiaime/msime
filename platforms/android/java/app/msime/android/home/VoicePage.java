package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.DoubaoAsrPolicy;
import app.msime.android.NativeClient;
import app.msime.android.ResourcePackService;
import app.msime.android.ResourcePacks;
import app.msime.android.VoiceConfiguration;
import app.msime.android.core.InputViewValuePolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import org.json.JSONObject;

/**
 * 语音输入页：识别语言、自动添加标点、离线识别、启动方式，以及隐私组的「上传语音以改进识别」。
 *
 * <p>键盘读同一批存储：语言是共享偏好的 `voice_input.language`（普通话 `zh-cn`、粤语 `yue`、英语 `en`、普通话 + 英语 `auto`）；自动标点绑定已有的 `voice_input.doubao_enable_punc`，只有豆包识别支持，其他识别器下置灰并写明原因；离线识别只有 Android 有，在 {@link AndroidLocalSettings} 的 `platform.android.voice_offline_fallback`；启动方式没有自己的字段，由本地设置的长按空格（`platform.android.space_voice`）和共享偏好的 `touch_voice_shortcut` 联合派生，选择时两个一起写。
 *
 * <p>离线识别要用的语音运行库（libonnxruntime 与 libsherpa-onnx-c-api）在 full 版里不随安装包，是按需下载的资源包 `voice-runtime`：打开离线识别时还没有运行库就经 {@link ResourcePackService} 下载（按流量计费的网络先确认），开关下面显示下载状态；安装包自带运行库时不需要它。
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

    /**
     * 页面渲染时读到的偏好与识别器。
     *
     * @param runtimeReady 本地语音运行库已可用：安装包自带，或 voice-runtime 资源包已装好
     * @param runtimePack 共享层列出的 voice-runtime 资源包（状态和下载大小），读不出来时为 null
     */
    private record State(JSONObject preferences, AndroidLocalSettings.Snapshot local, boolean punctuationSupported,
            boolean runtimeReady, @Nullable ResourcePacks.Pack runtimePack) {}

    @Nullable private LinearLayout column;
    @Nullable private GroupCard.Row contributeRow;
    @Nullable private State lastState;
    /** 运行库下载进度变了就重画；下载结束时重读一遍安装状态。 */
    private final ResourcePackService.Listener packListener = (pack, finished) -> {
        if (column == null || !ResourcePacks.VOICE_RUNTIME.equals(pack)) return;
        if (finished) reload();
        else if (lastState != null) render(lastState);
    };

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        ResourcePackService.addListener(packListener);
        reload();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        ResourcePackService.removeListener(packListener);
        column = null;
        contributeRow = null;
        lastState = null;
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
        ResourcePacks.Pack runtime = ResourcePacks.pack(context.getFilesDir(), ResourcePacks.VOICE_RUNTIME);
        boolean runtimeReady = (runtime != null && runtime.installed()) || bundledRuntime();
        return new State(preferences, AndroidLocalSettings.load(context),
            DoubaoAsrPolicy.PROVIDER.equals(configuration.provider()), runtimeReady, runtime);
    }

    /** 安装包自带语音运行库：按名字能载入。没带时载入失败，按需下载的资源包补上它。 */
    private static boolean bundledRuntime() {
        try {
            return NativeClient.localSpeechAvailable();
        } catch (LinkageError error) {
            return false;
        }
    }

    private void render(@Nullable State state) {
        LinearLayout target = column;
        if (target == null) return;
        lastState = state;
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
        String language = InputViewValuePolicy.textOr(voice, "language", "");
        GroupCard.Row[] languageRow = new GroupCard.Row[1];
        languageRow[0] = recognition.nav("识别语言", null, languageLabel(language),
            () -> pickLanguage(language, languageRow[0]));

        boolean punctuation = voice.optBoolean("doubao_enable_punc", true);
        GroupCard.Row punctuationRow = recognition.toggle("自动添加标点",
            state.punctuationSupported() ? null : "当前识别服务不支持，仅豆包语音识别可以自动加标点",
            punctuation, checked -> saveVoice("doubao_enable_punc", checked));
        punctuationRow.setEnabled(state.punctuationSupported());

        boolean offline = local.bool(AndroidLocalSettings.VOICE_OFFLINE_FALLBACK);
        recognition.toggle("离线识别", "无网络时使用本地模型，准确率略低。需要先安装本地语音模型", offline,
            checked -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.VOICE_OFFLINE_FALLBACK, checked, () -> {
                // 打开离线识别时先备好语音运行库，本地模型才跑得起来。
                if (checked && !state.runtimeReady()) requestRuntime(state);
                reload();
            }, this::reload));
        if (offline && !state.runtimeReady()) runtimeRow(recognition, state);

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

    /** 语音运行库的状态行：没下载时「下载」，下载中「取消」，校验中按钮置灰，失败「重试」。 */
    private void runtimeRow(GroupCard group, State state) {
        String pack = ResourcePacks.VOICE_RUNTIME;
        ResourcePackService.Status status = ResourcePackService.status(pack);
        String running = ResourcePackService.describe(status);
        long size = state.runtimePack() == null ? 0 : state.runtimePack().size();
        String subtitle = running != null ? running
            : "离线识别要用的运行库" + (size > 0 ? " · 需下载 " + ResourcePackService.size(size) : " · 需下载");
        if (status != null && status.phase() == ResourcePackService.Phase.VERIFYING) {
            group.button("本地语音运行库", subtitle, "校验中", () -> {}).setEnabled(false);
            return;
        }
        boolean downloading = status != null && status.running();
        group.button("本地语音运行库", subtitle, downloading ? "取消" : status == null ? "下载" : "重试", () -> {
            if (downloading) ResourcePackService.cancel(pack);
            else requestRuntime(state);
        });
    }

    private void requestRuntime(State state) {
        if (!isAdded()) return;
        long size = state.runtimePack() == null ? 0 : state.runtimePack().size();
        ResourcePackService.request(requireContext(), ResourcePacks.VOICE_RUNTIME, size);
    }

    private void saveLocal(String key, Object value) {
        KeyboardSheets.saveLocal(this, key, value, null, this::reload);
    }

    /** 长按空格在本地设置，工具栏按钮在共享偏好；先写本地，再写共享。 */
    private void saveTrigger(int trigger) {
        HostTask.run(this, context -> KeyboardSheets.writeLocal(context, AndroidLocalSettings.SPACE_VOICE,
                trigger == TRIGGER_SPACE)
            ? KeyboardSheets.write(context, preferences -> preferences.put("touch_voice_shortcut", trigger == TRIGGER_TOOLBAR))
            : null, this::afterSave);
    }

    private void save(KeyboardSheets.Edit edit) {
        HostTask.run(this, context -> KeyboardSheets.write(context, edit), this::afterSave);
    }

    /** 成功失败都重新渲染：选择行的点击回调捕获的是渲染时的值，不重建的话再次打开会勾着旧选项。 */
    private void afterSave(@Nullable JSONObject saved) {
        if (saved == null) MsToast.show(requireContext(), "保存失败，请重试");
        reload();
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
