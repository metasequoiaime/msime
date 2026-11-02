package app.msime.android.home;

import app.msime.android.ScreenCaptureProtection;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import android.text.InputType;
import android.widget.EditText;
import android.widget.LinearLayout;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import app.msime.android.AccountFailureText;
import app.msime.android.AccountStore;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.DeviceDataApi;
import app.msime.android.DoubaoAsrPolicy;
import app.msime.android.LocalVoiceModelPolicy;
import app.msime.android.LocalVoiceModels;
import app.msime.android.NativeClient;
import app.msime.android.OfflineImportPolicy;
import app.msime.android.ResourcePackService;
import app.msime.android.ResourcePacks;
import app.msime.android.TextPolicy;
import app.msime.android.VoiceConfiguration;
import app.msime.android.VoicePolishPolicy;
import app.msime.android.VoicePolishSettingsPolicy;
import app.msime.android.VoiceProviderCatalog;
import app.msime.android.VoiceProviderPolicy;
import app.msime.android.VoiceProviderSettings;
import app.msime.android.VoiceLanguageCatalog;
import app.msime.android.VoicePunctuation;
import app.msime.android.VoiceRecognitionActivity;
import app.msime.android.core.InputViewValuePolicy;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 语音输入页：识别服务及其密钥，识别语言、自动添加标点、语音标点、离线识别、启动方式，以及隐私组的「上传语音以改进识别」。
 *
 * <p>识别服务写共享偏好的 `voice_input.asr_provider`，密钥、接口和模型写同一对象里的 `asr_token` / `asr_tokens`、`asr_app_key`、`doubao_auth_mode`、`asr_endpoint`、`asr_model`、`asr_resource_id`，键盘经共享解析读它们，这里不另存一份。切换服务和凭据分槽的规则在 {@link VoiceProviderPolicy}。密钥只在本机：设置同步只带识别语言。
 *
 * <p>键盘读同一批存储：语言是共享偏好的 `voice_input.language`（普通话 `zh-cn`、粤语 `yue`、英语 `en`、普通话 + 英语 `auto`）；自动标点绑定已有的 `voice_input.doubao_enable_punc`，只决定豆包请求时要不要标点，其他识别器下置灰并指向「语音标点」；「语音标点」写共享偏好的 `voice_input.punctuation_mode`（{@link VoicePunctuation}），键盘和识别窗口在所有识别服务和润色之后、上屏之前按它处理；离线识别只有 Android 有，在 {@link AndroidLocalSettings} 的 `platform.android.voice_offline_fallback`；启动方式没有自己的字段，由本地设置的长按空格（`platform.android.space_voice`）和共享偏好的 `touch_voice_shortcut` 联合派生；组合项开启长按空格并保留工具栏选择，「无」关闭两个快捷入口。
 *
 * <p>本地识别（识别服务选「本地模型」，或打开离线识别）用 `voice_input.asr_model_path` 指向的模型。选了本地模型或打开离线识别时出现「本地模型」组：列出共享目录里手机可用的模型，可以下载、取消、选用和删除，下载经 {@link ResourcePackService} 排队，装在 `filesDir/voice-models/<id>`（{@link LocalVoiceModels}），下载完成且还没选用模型时自动选用。
 *
 * <p>本地识别要用的语音运行库（libonnxruntime 与 libsherpa-onnx-c-api）在 full 版里不随安装包，是按需下载的资源包 `voice-runtime`：选本地模型或打开离线识别时还没有运行库就经 {@link ResourcePackService} 下载（按流量计费的网络先确认），「本地模型」组里显示下载状态；安装包自带运行库时不需要它。
 *
 * <p>「上传语音以改进识别」（本地设置的 `platform.android.voice_contribute_audio`，只在本机、不同步）默认关闭；每次从关切到开都先弹确认框说明上传什么、保存多久，用户确认后才写（P19）。下面的「删除上传过的语音样本」经 Rust 的 voice_delete 删掉这台设备匿名账号名下的样本，不要求登录。
 */
public final class VoicePage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("识别", "服务", "润色", "隐私");
    }

    private static final String[] TRIGGER_LABELS = {"工具栏/长按空格", "无"};
    private static final int TRIGGER_COMBINED = 0;
    private static final int TRIGGER_NONE = 1;
    /** 「无」关闭两个快捷入口，识别配置及「更多工具」中的语音输入仍保留。 */
    static final String TRIGGER_NONE_HINT = "空格键和工具栏上不放语音入口，长按空格只输入空格；要说话可在「更多工具」里点「语音输入」";
    static final String TRIGGER_COMBINED_HINT = "选择后开启长按空格语音；工具栏按钮在「键盘 → 工具栏 → 语音」单独开关，两种方式可同时使用";
    static final String TRIGGER_SHEET_HINT = TRIGGER_COMBINED_HINT + "。选「无」时" + TRIGGER_NONE_HINT;
    /** 「语音标点」的「?」：四个选项各自的效果。 */
    static final String PUNCTUATION_MODE_HINT = "识别结果上屏前怎么处理标点，对所有识别服务和 AI 润色后的结果都生效。智能添加：识别出什么标点就上屏什么；省略句末标点：「你好。」上屏为「你好」；省略所有标点：「天啊！他是谁？」上屏为「天啊他是谁」；空格代替标点：上屏为「天啊 他是谁」。小数点、时间里的冒号和引号括号不受影响";
    static final String PUNCTUATION_MODE_SHEET_HINT = "对所有识别服务和 AI 润色后的结果生效，下一次识别起作用";
    /** 工具栏整体隐藏时，不替用户改变显示方式；长按空格与「更多工具」入口仍按各自规则可用。 */
    static final String TRIGGER_TOOLBAR_HIDDEN_HINT = "工具栏已隐藏，按钮暂不显示；可在「键盘 → 工具栏」更改显示方式，或在「更多工具」里点「语音输入」";

    /**
     * 页面渲染时读到的偏好与识别器。
     *
     * @param runtimeReady 本地语音运行库已可用：安装包自带，或 voice-runtime 资源包已装好
     * @param runtimePack 共享层列出的 voice-runtime 资源包（状态和下载大小），读不出来时为 null
     * @param resolvedProvider 共享解析给出的识别服务，没有可用配置（键盘用系统识别）时为 null
     * @param models 共享目录里手机可用的本地模型
     * @param modelsError 读模型目录失败时的错误码，读到了为 null
     * @param polishConfigured 共享解析给出了润色配置：开关打开时键盘会润色
     */
    private record State(JSONObject preferences, AndroidLocalSettings.Snapshot local, boolean punctuationSupported,
            boolean runtimeReady, @Nullable ResourcePacks.Pack runtimePack, @Nullable String resolvedProvider,
            List<LocalVoiceModels.Model> models, @Nullable String modelsError, boolean polishConfigured,
            boolean platformRecognizerAvailable, VoiceProviderCatalog catalog) {}

    /** 一次「测试」的结果：{@code message} 是共享层给的中文结果，不含凭据。 */

    @Nullable private GroupCard.Row contributeRow;
    @Nullable private State lastState;
    /** 正在测试的服务（`voice.asr` / `voice.polish`）：测试中再点同一行不再发请求，免得排起一串请求和对话框。 */
    private final Set<String> testing = new HashSet<>(2);
    /** 「从文件导入」等着文件选择器结果的那一项：资源包 id 或模型键、模型名（资源包为 null）和要的文件长度。 */
    private record ImportTarget(String pack, @Nullable String title, long[] sizes) {}
    @Nullable private ImportTarget pendingImport;
    private static final String SAVED_IMPORT_PACK = "voice.import.pack";
    private static final String SAVED_IMPORT_TITLE = "voice.import.title";
    private static final String SAVED_IMPORT_SIZES = "voice.import.sizes";
    private final ActivityResultLauncher<String[]> openDocuments =
        registerForActivityResult(new ActivityResultContracts.OpenMultipleDocuments(), this::onImportPicked);
    /** 运行库或本地模型的下载进度变了就重画；下载结束时重读一遍安装状态。 */
    private final ResourcePackService.Listener packListener = (pack, finished) -> {
        if (contentColumn() == null || !(ResourcePacks.VOICE_RUNTIME.equals(pack) || ResourcePackService.isVoiceModel(pack)))
            return;
        if (finished) reload();
        else if (lastState != null) render(lastState);
    };

    /** 文件选择器是另一个 Activity：它打开期间转屏或进程被回收，这个页面会重建，等着的那一项要跟着 Bundle 回来，否则选好的文件没有去处。 */
    @Override public void onCreate(@Nullable Bundle saved) {
        super.onCreate(saved);
        String pack = saved == null ? null : saved.getString(SAVED_IMPORT_PACK);
        long[] sizes = saved == null ? null : saved.getLongArray(SAVED_IMPORT_SIZES);
        if (pack != null && sizes != null) pendingImport = new ImportTarget(pack, saved.getString(SAVED_IMPORT_TITLE), sizes);
    }

    @Override public void onSaveInstanceState(@NonNull Bundle out) {
        super.onSaveInstanceState(out);
        ImportTarget target = pendingImport;
        if (target == null) return;
        out.putString(SAVED_IMPORT_PACK, target.pack());
        out.putString(SAVED_IMPORT_TITLE, target.title());
        out.putLongArray(SAVED_IMPORT_SIZES, target.sizes());
    }

    @Override protected void onContentReady() {
        ResourcePackService.addListener(packListener);
    }

    @Override protected void onContentDestroyed() {
        ResourcePackService.removeListener(packListener);
        contributeRow = null;
        lastState = null;
        // HostTask fences its callback to this view, so a test still in flight when the view goes away never reports back; the marker must not lock the rebuilt page out of testing again.
        testing.clear();
    }

    @Override protected void reload() {
        HostTask.run(this, VoicePage::read, this::render);
    }

    @Nullable private static State read(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        if (preferences == null) return null;
        VoiceConfiguration configuration = VoiceConfiguration.read(HostStore.directory(context), "settings");
        ResourcePacks.Pack runtime = ResourcePacks.pack(context.getFilesDir(), ResourcePacks.VOICE_RUNTIME);
        boolean runtimeReady = (runtime != null && runtime.installed()) || bundledRuntime();
        List<LocalVoiceModels.Model> models = List.of();
        String modelsError = null;
        try {
            models = LocalVoiceModels.list(context.getFilesDir());
        } catch (ResourcePacks.Failure error) {
            modelsError = error.code();
        }
        return new State(preferences, AndroidLocalSettings.load(context),
            // 按用户选的服务判断，而不是按解析结果：选了豆包还没填密钥时开关照样能设，填好后生效，不该说「当前识别服务不支持」。
            DoubaoAsrPolicy.PROVIDER.equals(InputViewValuePolicy.textOr(preferences.optJSONObject("voice_input"),
                "asr_provider", "")), runtimeReady, runtime, configuration.provider(),
            models, modelsError, configuration.polish() != null,
            VoiceRecognitionActivity.available(context), VoiceProviderCatalog.read());
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
        LinearLayout target = contentColumn();
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

        serviceGroup(target, voice, state);
        boolean offline = local.bool(AndroidLocalSettings.VOICE_OFFLINE_FALLBACK);
        if (VoiceProviderPolicy.LOCAL.equals(InputViewValuePolicy.textOr(voice, "asr_provider", "")) || offline)
            modelsGroup(target, voice, state);

        GroupCard recognition = GroupCard.add(sectionColumn(target, "识别"), "识别");
        String language = InputViewValuePolicy.textOr(voice, "language", "");
        GroupCard.Row[] languageRow = new GroupCard.Row[1];
        languageRow[0] = recognition.nav("识别语言", null, languageLabel(language),
            () -> pickLanguage(language, languageRow[0]));

        boolean punctuation = voice.optBoolean("doubao_enable_punc", true);
        GroupCard.Row punctuationRow = recognition.toggle("自动添加标点",
            state.punctuationSupported() ? null : "只对豆包生效；其他识别服务的标点用下面的「语音标点」",
            punctuation, checked -> saveVoice("doubao_enable_punc", checked))
            .help("只决定豆包识别要不要在结果里加标点。本地模型、系统识别和其他服务自己会加标点，要去掉或替换它们，用下面的「语音标点」");
        punctuationRow.setEnabled(state.punctuationSupported());

        String punctuationMode = VoicePunctuation.mode(voice.opt("punctuation_mode"));
        GroupCard.Row[] punctuationModeRow = new GroupCard.Row[1];
        punctuationModeRow[0] = recognition.nav("语音标点", null, VoicePunctuation.label(punctuationMode),
            () -> pickPunctuationMode(punctuationMode, punctuationModeRow[0])).help(PUNCTUATION_MODE_HINT);

        recognition.toggle("离线识别", null, offline,
            checked -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.VOICE_OFFLINE_FALLBACK, checked, () -> {
                // 打开离线识别时先备好语音运行库，本地模型才跑得起来。
                if (checked && !state.runtimeReady()) requestRuntime(state);
                reload();
            }, this::reload)).help("无网络时改用选用的本地模型，准确率略低");

        int trigger = trigger(local.bool(AndroidLocalSettings.SPACE_VOICE),
            preferences.optBoolean("touch_voice_shortcut", false));
        // 两种用法的说明收进「?」；工具栏整体隐藏、按钮暂不显示是当前状态，留在副标题里一眼看到。
        String triggerHint = trigger == TRIGGER_NONE ? TRIGGER_NONE_HINT : TRIGGER_COMBINED_HINT;
        String triggerWarning = trigger != TRIGGER_NONE && preferences.optBoolean("touch_voice_shortcut", false)
            && local.bool(AndroidLocalSettings.TOOLBAR_HIDDEN) ? TRIGGER_TOOLBAR_HIDDEN_HINT : null;
        GroupCard.Row[] triggerRow = new GroupCard.Row[1];
        triggerRow[0] = recognition.nav("启动方式", triggerWarning,
            TRIGGER_LABELS[trigger],
            () -> pickTrigger(trigger, triggerRow[0])).help(triggerHint);

        recognition.slider("本地停顿时间", null,
            AndroidLocalSettings.VOICE_PAUSE_MIN_MILLIS, AndroidLocalSettings.VOICE_PAUSE_MAX_MILLIS, 100,
            local.integer(AndroidLocalSettings.VOICE_PAUSE_MILLIS), value -> value + " 毫秒",
            value -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.VOICE_PAUSE_MILLIS,
                value, this::reload, this::reload)).help("本地识别出字后停顿这么久就结束录音；按住说话时由松手结束，锁定录音时由发送或取消结束");
        recognition.slider("判停灵敏度", null,
            10, 90, 1, local.integer(AndroidLocalSettings.VOICE_VAD_SENSITIVITY), value -> value + "%",
            value -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.VOICE_VAD_SENSITIVITY,
                value, this::reload, this::reload)).help("数值越高越容易判定说完、结束录音；调得太高，轻声说的字可能被当成停顿");
        recognition.help("停顿时间仅用于键盘内的本地识别；灵敏度仅用于点按开始、带语音活动检测的本地整段模型，按住空格时不变。系统识别自行判停，云服务仍由「说完了」或松开空格结束。这两项只保存在本机，下次本地识别生效。");

        polishGroup(target, voice, state);

        GroupCard privacy = GroupCard.add(sectionColumn(target, "隐私"), "隐私");
        boolean contribute = local.bool(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO);
        contributeRow = privacy.toggle("上传语音以改进识别", null, contribute,
            this::onContributeChanged).help("语音片段匿名处理，可随时关闭");
        privacy.button("删除上传过的语音样本", "删掉这台设备之前上传的语音和识别文字，不需要登录", "删除",
            this::confirmDeleteVoiceSamples);
    }

    /**
     * 删掉这台设备匿名账号名下上传过的语音样本（Rust 的 voice_delete，#8201 迁移计划 5c）：语音贡献只用匿名账号，从没登录过的用户也能删；注销账号或删除云端数据时那一步没做成的，也在这里重试。服务端要求最近登录，Rust 会用保存的匿名身份重新登录一次再删，这一次计入每天的匿名登录限额。
     */
    private void confirmDeleteVoiceSamples() {
        MessageDialog.confirm(requireContext(), "删除上传过的语音样本？",
            "这台设备之前上传的语音和识别出的文字会从水杉云删掉，无法恢复。以后是否继续上传由上面的开关决定。",
            "删除", () -> HostTask.runNetwork(this, context -> {
                try {
                    DeviceDataApi.deleteVoiceSamples(context);
                    return "";
                } catch (AccountStore.Failure failure) {
                    return AccountFailureText.voiceDelete(failure.code, failure.retryAfterSeconds);
                }
            }, failure -> MsToast.show(requireContext(), failure == null || !failure.isEmpty()
                ? (failure == null ? "没有删掉，请稍后重试" : failure) : "已删除上传过的语音样本")));
    }

    /** 「识别服务」组：服务选择，选了联网服务时再列出它要的密钥和可改的接口、模型。 */
    private void serviceGroup(LinearLayout target, JSONObject voice, State state) {
        String provider = InputViewValuePolicy.textOr(voice, "asr_provider", "");
        GroupCard service = GroupCard.add(sectionColumn(target, "服务"), "识别服务");
        String status = VoiceProviderPolicy.LOCAL.equals(provider)
            ? VoiceProviderPolicy.localStatus(selectedModel(voice, state) != null, state.runtimeReady())
            : VoiceProviderPolicy.status(provider, state.resolvedProvider(), state.platformRecognizerAvailable());
        if (!state.catalog().available()) service.note("服务目录暂不可用，请稍后返回重试。");
        service.nav("识别服务", status, state.catalog().asrLabel(provider), () -> pickProvider(provider, state))
            .setEnabled(state.catalog().available());
        if (!VoiceProviderPolicy.network(provider)) return;

        String box = TextPolicy.trimUnicodeWhitespace(InputViewValuePolicy.textOr(voice, "asr_token", ""));
        JSONObject slots = voice.optJSONObject("asr_tokens");
        boolean hasToken = hasToken(box, slots, provider);
        String endpoint = InputViewValuePolicy.textOr(voice, "asr_endpoint", "").trim();
        if (DoubaoAsrPolicy.isStreaming(provider)) {
            boolean legacy = VoiceProviderPolicy.legacyAuth(InputViewValuePolicy.textOr(voice, "doubao_auth_mode", ""));
            service.nav("鉴权方式", null, legacy ? "App ID + Access Token" : "API Key", () -> pickAuthMode(legacy));
            if (legacy) {
                String appId = TextPolicy.trimUnicodeWhitespace(InputViewValuePolicy.textOr(voice, "asr_app_key", ""));
                service.nav("App ID", null, appId.isEmpty() ? "未设置" : "已设置",
                    () -> editAppId(provider, appId));
            }
            String tokenTitle = legacy ? "Access Token" : "API Key";
            service.nav(tokenTitle, null, hasToken ? "已设置" : "未设置", () -> editToken("asr", provider, tokenTitle));
            String endpointLabel = state.catalog().available() ? state.catalog().doubaoEndpointLabel(endpoint)
                : endpoint.isEmpty() ? "默认" : HomeTextPolicy.shortLabel(endpoint);
            service.nav("识别接口", VoiceProviderPolicy.endpointProblem(provider, endpoint),
                endpointLabel, () -> pickDoubaoEndpoint(provider, endpoint, state.catalog()))
                .setEnabled(state.catalog().available());
            String resource = InputViewValuePolicy.textOr(voice, "asr_resource_id", "").trim();
            service.nav("资源 ID", null, resource.isEmpty() ? "默认" : HomeTextPolicy.shortLabel(resource), () -> editText("资源 ID",
                "火山引擎控制台里开通的识别资源；留空使用 volc.seedasr.sauc.duration", resource,
                value -> saveVoice("asr_resource_id", value)));
            testRow(service, "voice.asr");
            service.help("在火山引擎控制台开通豆包流式语音识别后获取密钥。密钥只保存在本机，不随设置同步。");
            return;
        }
        service.nav("API Key", null, hasToken ? "已设置" : "未设置", () -> editToken("asr", provider, "API Key"));
        String model = InputViewValuePolicy.textOr(voice, "asr_model", "").trim();
        service.nav("模型", null, model.isEmpty() ? "默认" : HomeTextPolicy.shortLabel(model), () -> editText("模型",
            "留空使用该服务的默认识别模型", model, value -> saveVoice("asr_model", value)));
        String problem = VoiceProviderPolicy.endpointProblem(provider, endpoint);
        service.nav("接口地址", problem, endpoint.isEmpty() ? "默认" : HomeTextPolicy.shortLabel(endpoint),
            () -> editEndpoint("asr", provider, endpoint));
        testRow(service, "voice.asr");
        service.help("说完后录音上传到所选服务识别。API Key 只保存在本机，不随设置同步。");
    }

    private void pickProvider(String selected, State state) {
        OptionSheet sheet = new OptionSheet(requireContext(), "识别服务", null);
        String current = selected.isEmpty() ? VoiceProviderPolicy.SYSTEM : selected;
        for (VoiceProviderCatalog.Provider entry : state.catalog().asr()) {
            String provider = entry.id();
            sheet.option(entry.title(), provider.equals(current), () -> {
                if (provider.equals(current)) return;
                saveProvider(provider, firstInstalled(state));
                // 本地识别要运行库，选它时就开始备好，和打开离线识别一样。
                if (VoiceProviderPolicy.LOCAL.equals(provider) && !state.runtimeReady()) requestRuntime(state);
            });
        }
        sheet.show();
    }

    /**
     * 换服务：共享 Rust 计算凭据换槽并清空接口和模型，运行时解析再补默认值。换到本地模型而还没选用模型时，选用已经装好的第一个。
     *
     * @param installedModel 已装好的第一个本地模型目录，没有时为 null
     */
    private void saveProvider(String provider, @Nullable String installedModel) {
        savePreferences(preferences -> {
            JSONObject voice = KeyboardSheets.child(preferences, "voice_input");
            String from = InputViewValuePolicy.textOr(voice, "asr_provider", "");
            if (provider.equals(from)) return;
            VoiceProviderSettings.switchProvider(voice, "asr", provider);
            if (VoiceProviderPolicy.LOCAL.equals(provider) && installedModel != null
                    && InputViewValuePolicy.textOr(voice, "asr_model_path", "").trim().isEmpty())
                voice.put("asr_model_path", installedModel);
        });
    }

    /** 「服务」分页里识别服务下面的本地识别部分：先说明本地识别缺哪一项，再列出模型的下载、取消、选用和删除。本地模型本身就是一种识别服务，和服务选择放在同一个分页，页面最多四个分页。 */
    private void modelsGroup(LinearLayout target, JSONObject voice, State state) {
        LinearLayout column = sectionColumn(target, "服务");
        GroupCard preparation = GroupCard.add(column, "本地识别准备");
        LocalVoiceModels.Model chosenModel = selectedModel(voice, state);
        preparation.value("已选模型", null, chosenModel == null ? "未选用" : chosenModel.title());
        if (state.runtimeReady()) preparation.value("本地语音运行库", null, "已就绪");
        else {
            runtimeRow(preparation, state);
            ResourcePacks.Pack runtime = state.runtimePack();
            ResourcePackService.Status status = ResourcePackService.status(ResourcePacks.VOICE_RUNTIME);
            if (runtime != null && !runtime.importFiles().isEmpty() && (status == null || !status.running())) {
                preparation.nav("从文件导入运行库", null, null,
                    () -> explainImport(ResourcePacks.VOICE_RUNTIME, null,
                        ResourcePackService.title(ResourcePacks.VOICE_RUNTIME), runtime.importFiles())).help("网络不通时，用自己下载好的运行库安装");
            }
        }
        // 都准备好了是状态，留在卡片下方；还缺东西时的解释收进组标题的「?」。
        if (chosenModel != null && state.runtimeReady()) preparation.footer("模型和运行库均已准备好，可以开始语音输入。");
        else preparation.help("本地识别需要模型和语音运行库。导入模型后，还需单独准备运行库；「已选用」只表示选中了模型。");
        GroupCard group = GroupCard.add(column, "本地模型");
        if (state.modelsError() != null) {
            group.note("读取模型列表失败（" + state.modelsError() + "），请稍后返回重试。");
            return;
        }
        String chosen = InputViewValuePolicy.textOr(voice, "asr_model_path", "");
        for (LocalVoiceModels.Model model : state.models()) {
            String key = ResourcePackService.voiceModelKey(model.id());
            ResourcePackService.Status status = ResourcePackService.status(key);
            String running = ResourcePackService.describe(status);
            boolean selected = LocalVoiceModelPolicy.selected(model.path(), chosen);
            Runnable start = () -> ResourcePackService.requestVoiceModel(requireContext(), model.id(), model.title(),
                model.archiveSize());
            switch (LocalVoiceModelPolicy.action(model.installed(), download(status))) {
                case MANAGE -> group.nav(model.title(), model.description().isEmpty() ? null : model.description(),
                    LocalVoiceModelPolicy.installedValue(selected), () -> manageModel(model, selected));
                case DOWNLOAD -> group.button(model.title(),
                    LocalVoiceModelPolicy.summary(model.description(), model.archiveSize(), model.memory()), "下载", start);
                case RETRY -> group.button(model.title(), running, "重试", start);
                case CANCEL -> group.button(model.title(), running, "取消", () -> ResourcePackService.cancel(key));
                case VERIFYING -> group.button(model.title(), running, "校验中", () -> {}).setEnabled(false);
            }
        }
        List<Runnable> imports = new ArrayList<>();
        List<String> importLabels = new ArrayList<>();
        for (LocalVoiceModels.Model model : state.models()) {
            String key = ResourcePackService.voiceModelKey(model.id());
            ResourcePackService.Status status = ResourcePackService.status(key);
            if (model.installed() || model.importFiles().isEmpty() || (status != null && status.running())) continue;
            importLabels.add(model.title());
            imports.add(() -> explainImport(key, model.title(), model.title(), model.importFiles()));
        }
        if (!imports.isEmpty()) {
            group.nav("从文件导入", null, null, () -> {
                OptionSheet sheet = new OptionSheet(requireContext(), "从文件导入", null);
                for (int index = 0; index < imports.size(); index++) {
                    sheet.option(importLabels.get(index), false, imports.get(index));
                }
                sheet.show();
            }).help("网络不通时，用自己下载好的文件安装");
        }
        group.help("本地模型在手机上识别，录音不离开本机。模型较大，建议连上 Wi-Fi 再下载；下载完成且还没有选用模型时会自动选用。");
    }

    /** 说明要自己下载哪些文件，再打开文件选择器；「下载地址」用浏览器打开上游地址。 */
    private void explainImport(String pack, @Nullable String modelTitle, String title, List<ResourcePacks.ImportFile> files) {
        List<String> lines = new ArrayList<>(files.size());
        long[] sizes = new long[files.size()];
        for (int index = 0; index < files.size(); index++) {
            ResourcePacks.ImportFile file = files.get(index);
            lines.add(OfflineImportPolicy.fileLine(file.name(), file.size()));
            sizes[index] = file.size();
        }
        ScreenCaptureProtection.show(new MaterialAlertDialogBuilder(requireContext())
            .setTitle("导入" + title)
            .setMessage(OfflineImportPolicy.instructions(lines))
            .setNegativeButton("取消", null)
            .setNeutralButton("下载地址", (dialog, which) -> openDownloads(files))
            .setPositiveButton("选择文件", (dialog, which) -> {
                pendingImport = new ImportTarget(pack, modelTitle, sizes);
                try {
                    openDocuments.launch(new String[] {"*/*"});
                } catch (ActivityNotFoundException error) {
                    pendingImport = null;
                    MsToast.show(requireContext(), "这台设备没有可用的文件选择器");
                }
            })
            .create());
    }

    private void openDownloads(List<ResourcePacks.ImportFile> files) {
        if (files.size() == 1) {
            openUrl(files.get(0).url());
            return;
        }
        OptionSheet sheet = new OptionSheet(requireContext(), "下载地址", null);
        for (ResourcePacks.ImportFile file : files) sheet.option(file.name(), false, () -> openUrl(file.url()));
        sheet.show();
    }

    private void openUrl(String url) {
        if (url.isEmpty()) return;
        if (!LinkActions.open(requireContext(), url)) {
            MsToast.show(requireContext(), "没有可以打开链接的应用");
        }
    }

    /** 文件选择器的结果：交给下载服务复制并安装；用户关掉选择器时什么也不做。 */
    private void onImportPicked(@Nullable List<Uri> uris) {
        ImportTarget target = pendingImport;
        pendingImport = null;
        if (target == null || uris == null || uris.isEmpty() || !isAdded()) return;
        ResourcePackService.requestImport(requireContext(), target.pack(), target.title(), uris, target.sizes());
    }

    private static LocalVoiceModelPolicy.Download download(@Nullable ResourcePackService.Status status) {
        if (status == null) return LocalVoiceModelPolicy.Download.NONE;
        return switch (status.phase()) {
            case DOWNLOADING, IMPORTING -> LocalVoiceModelPolicy.Download.DOWNLOADING;
            case VERIFYING -> LocalVoiceModelPolicy.Download.VERIFYING;
            case FAILED -> LocalVoiceModelPolicy.Download.FAILED;
        };
    }

    /** 已装模型：没在用的可以选用；都可以删除，删除成功后按当时的当前选择清理 `asr_model_path`。 */
    private void manageModel(LocalVoiceModels.Model model, boolean selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), model.title(), null);
        if (!selected) sheet.option("使用这个模型", false, () -> saveVoice("asr_model_path", model.path()));
        sheet.option("删除", false, () -> MessageDialog.confirm(requireContext(),
            "删除「" + model.title() + "」？",
            selected ? "这是正在使用的模型，删除后本地识别和离线识别要重新下载或选用别的模型。" : "删除后要用时需要重新下载。",
            "删除", () -> removeModel(model)));
        sheet.show();
    }

    private void removeModel(LocalVoiceModels.Model model) {
        HostTask.run(this, context -> {
            try {
                LocalVoiceModels.remove(context, model.id());
            } catch (ResourcePacks.Failure error) {
                return error.code();
            }
            KeyboardSheets.write(context, preferences -> {
                JSONObject voice = preferences.optJSONObject("voice_input");
                String chosen = InputViewValuePolicy.textOr(voice, "asr_model_path", "");
                String next = LocalVoiceModelPolicy.selectionAfterRemoval(model.path(), chosen);
                if (chosen.equals(next)) return;
                voice = KeyboardSheets.child(preferences, "voice_input");
                voice.put("asr_model_path", next);
            });
            return "";
        }, failure -> {
            if (!failure.isEmpty()) MsToast.show(requireContext(), "删除失败（" + failure + "）");
            reload();
        });
    }

    /** 偏好里选用的、而且已经装好的模型；没有时为 null。 */
    @Nullable private static LocalVoiceModels.Model selectedModel(JSONObject voice, State state) {
        String chosen = InputViewValuePolicy.textOr(voice, "asr_model_path", "");
        for (LocalVoiceModels.Model model : state.models()) {
            if (model.installed() && LocalVoiceModelPolicy.selected(model.path(), chosen)) return model;
        }
        return null;
    }

    @Nullable private static String firstInstalled(State state) {
        for (LocalVoiceModels.Model model : state.models()) if (model.installed()) return model.path();
        return null;
    }

    private void pickAuthMode(boolean legacy) {
        OptionSheet sheet = new OptionSheet(requireContext(), "鉴权方式", "以火山引擎控制台给出的凭据类型为准");
        sheet.option("API Key（新版控制台）", !legacy,
            () -> saveVoice("doubao_auth_mode", VoiceProviderPolicy.AUTH_API_KEY));
        sheet.option("App ID + Access Token（旧版控制台）", legacy,
            () -> saveVoice("doubao_auth_mode", VoiceProviderPolicy.AUTH_LEGACY));
        sheet.show();
    }

    private void pickDoubaoEndpoint(String provider, String endpoint, VoiceProviderCatalog catalog) {
        VoiceProviderCatalog.StreamEndpoint selected = catalog.selectedEndpoint(endpoint);
        OptionSheet sheet = new OptionSheet(requireContext(), "识别接口", null);
        for (VoiceProviderCatalog.StreamEndpoint entry : catalog.doubaoEndpoints())
            sheet.option(entry.title(), entry.equals(selected), () -> saveVoice("asr_endpoint", entry.endpoint()));
        sheet.option("自定义地址…", selected == null, () -> editEndpoint("asr", provider, endpoint));
        sheet.show();
    }

    /**
     * 密钥框不回显旧值；留空删除这个服务的密钥（当前框和它的槽一起清）。确认时服务已被键盘或别的页面换掉就不写。
     *
     * @param prefix 字段前缀：识别是 {@code asr}（`asr_provider`、`asr_token`、`asr_tokens`），润色是 {@code polish}
     */
    private void editToken(String prefix, String provider, String title) {
        VoiceProviderCatalog catalog = lastState == null ? VoiceProviderCatalog.unavailable() : lastState.catalog();
        String label = "asr".equals(prefix) ? catalog.asrLabel(provider) : catalog.polishLabel(provider);
        InputDialog dialog = new InputDialog(requireContext(), title, label + " 的" + title + "，只保存在本机");
        EditText field = dialog.addField("留空表示删除", "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        dialog.setValidator(values -> {
            boolean valid = VoiceProviderPolicy.validCredential(provider, values.get(0));
            field.setError(valid ? null : "含有不能用的字符");
            return valid;
        });
        dialog.setPrimary("保存", values -> savePreferences(preferences -> {
            JSONObject voice = KeyboardSheets.child(preferences, "voice_input");
            if (!provider.equals(InputViewValuePolicy.textOr(voice, prefix + "_provider", "")))
                throw new JSONException("provider changed");
            voice.put(prefix + "_token", TextPolicy.trimUnicodeWhitespace(values.get(0)));
            KeyboardSheets.child(voice, prefix + "_tokens").remove(provider);
        }));
        dialog.show();
    }

    private void editAppId(String provider, String current) {
        InputDialog dialog = new InputDialog(requireContext(), "App ID", "火山引擎控制台里应用的 App ID");
        EditText field = dialog.addField("App ID", current, InputType.TYPE_CLASS_TEXT);
        dialog.setValidator(values -> {
            boolean valid = VoiceProviderPolicy.validCredential(provider, values.get(0));
            field.setError(valid ? null : "含有不能用的字符");
            return valid;
        });
        dialog.setPrimary("保存", values -> saveVoice("asr_app_key", TextPolicy.trimUnicodeWhitespace(values.get(0))));
        dialog.show();
    }

    /** @param prefix 字段前缀，写 `asr_endpoint` 或 `polish_endpoint` */
    private void editEndpoint(String prefix, String provider, String current) {
        boolean streaming = DoubaoAsrPolicy.isStreaming(provider);
        InputDialog dialog = new InputDialog(requireContext(), "接口地址",
            streaming ? "豆包流式识别的 wss:// 地址" : "留空使用该服务的默认地址；代理或专属地址填完整的 https:// 地址");
        EditText field = dialog.addField(streaming ? "wss://" : "https://", current,
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        dialog.setValidator(values -> {
            String problem = VoiceProviderPolicy.endpointProblem(provider, values.get(0));
            field.setError(problem);
            return problem == null;
        });
        dialog.setPrimary("保存", values -> saveVoice(prefix + "_endpoint", values.get(0).trim()));
        dialog.show();
    }

    private interface TextAction {
        void accept(String value);
    }

    private void editText(String title, String message, String current, TextAction save) {
        InputDialog dialog = new InputDialog(requireContext(), title, message);
        dialog.addField(title, current, InputType.TYPE_CLASS_TEXT);
        dialog.setValidator(values -> !TextPolicy.hasControl(values.get(0)));
        dialog.setPrimary("保存", values -> save.accept(values.get(0).trim()));
        dialog.show();
    }

    private static boolean hasToken(String direct, @Nullable JSONObject slots, String provider) {
        return !direct.isEmpty() || (slots != null
            && !TextPolicy.trimUnicodeWhitespace(slots.optString(provider, "")).isEmpty());
    }

    /** 「润色」组：开关，打开后是服务、密钥、模型、接口、方案和「测试」。键盘读的同样是这些 `polish_*` 字段。 */
    private void polishGroup(LinearLayout target, JSONObject voice, State state) {
        GroupCard polish = GroupCard.add(sectionColumn(target, "润色"), "润色");
        boolean enabled = VoicePolishPolicy.requested(voice.optBoolean("polish_enabled", false),
            voice.optBoolean("polish_text", false));
        String provider = InputViewValuePolicy.textOr(voice, "polish_provider", "");
        String box = TextPolicy.trimUnicodeWhitespace(InputViewValuePolicy.textOr(voice, "polish_token", ""));
        JSONObject slots = voice.optJSONObject("polish_tokens");
        boolean hasToken = hasToken(box, slots, provider);
        String status = VoicePolishSettingsPolicy.status(enabled, state.polishConfigured(), hasToken);
        // 两个开关在共享设置页一起写，任一个开着键盘就会润色，所以关的时候两个都关。
        polish.toggle("润色识别结果", status, enabled, checked -> savePreferences(
            preferences -> {
                JSONObject next = KeyboardSheets.child(preferences, "voice_input");
                next.put("polish_text", checked);
                next.put("polish_enabled", checked);
            })).help("识别后交给大模型整理文字再上屏");
        if (!enabled) return;
        polish.nav("润色服务", state.catalog().available() ? null : "服务目录暂不可用，请稍后返回重试。",
            state.catalog().polishLabel(provider), () -> pickPolishProvider(provider, state.catalog()))
            .setEnabled(state.catalog().available());
        polish.nav("润色 API Key", null, hasToken ? "已设置" : "未设置", () -> editToken("polish", provider, "API Key"));
        String model = InputViewValuePolicy.textOr(voice, "polish_model", "").trim();
        polish.nav("润色模型", null, model.isEmpty() ? "默认" : HomeTextPolicy.shortLabel(model), () -> editText("润色模型",
            "留空使用该服务的默认模型", model, value -> saveVoice("polish_model", value)));
        String endpoint = InputViewValuePolicy.textOr(voice, "polish_endpoint", "").trim();
        polish.nav("润色接口地址", VoiceProviderPolicy.endpointProblem(provider, endpoint),
            endpoint.isEmpty() ? "默认" : HomeTextPolicy.shortLabel(endpoint), () -> editEndpoint("polish", provider, endpoint));
        String prompt = InputViewValuePolicy.textOr(voice, "polish_prompt_id", "");
        polish.nav("润色方案", null, VoicePolishSettingsPolicy.promptLabel(prompt), () -> pickPrompt(prompt));
        String field = VoicePolishSettingsPolicy.customField(prompt);
        if (field != null) {
            String custom = InputViewValuePolicy.textOr(voice, field, "");
            polish.nav("方案内容", null, custom.trim().isEmpty() ? "未填写" : HomeTextPolicy.shortLabel(custom.trim()),
                () -> editCustomPrompt(field, custom));
        }
        testRow(polish, "voice.polish");
        polish.help("识别出的文字会发给所选服务整理后再上屏。API Key 只保存在本机，不随设置同步。");
    }

    private void pickPolishProvider(String selected, VoiceProviderCatalog catalog) {
        OptionSheet sheet = new OptionSheet(requireContext(), "润色服务", null);
        String current = selected.isEmpty() && !catalog.polish().isEmpty() ? catalog.polish().get(0).id() : selected;
        for (VoiceProviderCatalog.Provider entry : catalog.polish()) {
            String provider = entry.id();
            sheet.option(entry.title(), provider.equals(current), () -> {
                if (!provider.equals(current)) savePolishProvider(provider);
            });
        }
        sheet.show();
    }

    /** 换润色服务：与识别服务相同，凭据换槽，接口和模型清空交给共享层补该服务的默认值。 */
    private void savePolishProvider(String provider) {
        savePreferences(preferences -> {
            JSONObject voice = KeyboardSheets.child(preferences, "voice_input");
            String from = InputViewValuePolicy.textOr(voice, "polish_provider", "");
            if (provider.equals(from)) return;
            VoiceProviderSettings.switchProvider(voice, "polish", provider);
        });
    }

    private void pickPrompt(String selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "润色方案", "自定义方案可以写自己的提示词");
        String current = selected.isEmpty() ? VoicePolishSettingsPolicy.PROMPTS[0] : selected;
        for (String id : VoicePolishSettingsPolicy.PROMPTS) {
            sheet.option(VoicePolishSettingsPolicy.promptLabel(id), id.equals(current),
                () -> saveVoice("polish_prompt_id", id));
        }
        sheet.show();
    }

    private void editCustomPrompt(String field, String current) {
        InputDialog dialog = new InputDialog(requireContext(), "方案内容",
            "写给大模型的提示词，识别出的文字会放在 <asr_text> 标签里交给它");
        dialog.addField("提示词", current,
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        dialog.setValidator(values -> !TextPolicy.hasControlExceptNewline(values.get(0)));
        dialog.setPrimary("保存", values -> saveVoice(field, values.get(0).trim()));
        dialog.show();
    }

    /** 「测试」行：按存着的配置在网络线程上发一次请求，结果用对话框给出（文案可能超过 Toast 的两行）。请求最长约 15 秒，不能占着设置页读写偏好的那条串行线程。 */
    private void testRow(GroupCard group, String service) {
        group.button("测试连接", null, "测试", () -> {
            MsToast.show(requireContext(), "正在测试…");
            if (!testing.add(service)) return;
            HostTask.runNetwork(this, context -> ServiceCredentialTest.voice(HostStore.directory(context), service), result -> {
                testing.remove(service);
                if (!isAdded()) return;
                MessageDialog.show(requireContext(), VoicePolishSettingsPolicy.testTitle(result.ok()), result.message());
            });
        }).help("用现在保存的设置发一次请求，最长约 15 秒");
    }

    private void pickLanguage(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "识别语言", null);
        String current = selected.isEmpty() ? VoiceLanguageCatalog.defaultValue() : selected;
        for (VoiceLanguageCatalog.Language language : VoiceLanguageCatalog.mobile()) {
            String value = language.value();
            String label = language.title();
            sheet.option(label, value.equals(current), () -> {
                row.setValue(label);
                saveVoice("language", value);
            });
        }
        sheet.show();
    }

    private void pickPunctuationMode(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "语音标点", PUNCTUATION_MODE_SHEET_HINT);
        for (int index = 0; index < VoicePunctuation.MODES.length; index++) {
            String value = VoicePunctuation.MODES[index];
            String label = VoicePunctuation.LABELS[index];
            sheet.option(label, value.equals(selected), () -> {
                row.setValue(label);
                saveVoice("punctuation_mode", value);
            });
        }
        sheet.show();
    }

    private void pickTrigger(int selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "启动方式", TRIGGER_SHEET_HINT);
        for (int index = 0; index < TRIGGER_LABELS.length; index++) {
            int choice = index;
            sheet.option(TRIGGER_LABELS[index], index == selected, () -> {
                // 仅工具栏开启时也显示组合项，明确选择它仍应开启长按空格，同时保留工具栏开关。
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
            saveLocalSetting(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO, false);
            return;
        }
        if (row != null) row.setChecked(false);
        MessageDialog.confirm(requireContext(), "上传语音以改进识别？",
            "开启后，每次语音输入的音频和识别出的文字会上传到水杉云，只用于改进语音识别。数据保存 180 天，到期自动删除；你可以随时在这里关闭。隐私模式、密码框和不允许个性化学习的输入框里不会上传。",
            "开启", () -> {
                GroupCard.Row current = contributeRow;
                if (current != null) current.setChecked(true);
                saveLocalSetting(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO, true);
            });
    }

    private void saveVoice(String member, Object value) {
        savePreferences(preferences -> {
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

    /** 组合项只开启长按空格，工具栏独立控制；「无」先关闭本地长按，再关闭共享工具栏入口。 */
    private void saveTrigger(int trigger) {
        HostTask.run(this, context -> KeyboardSheets.writeLocal(context, AndroidLocalSettings.SPACE_VOICE,
                trigger == TRIGGER_COMBINED)
            ? KeyboardSheets.write(context, preferences -> {
                if (trigger == TRIGGER_NONE) preferences.put("touch_voice_shortcut", false);
            })
            : null, this::afterSave);
    }

    /** 本地与共享写入失败时先提示，再重读实际状态。 */
    private void afterSave(@Nullable JSONObject saved) {
        if (saved == null) MsToast.show(requireContext(), "保存失败，请重试");
        reload();
    }

    /** 任一快捷入口开启时显示组合项，不在读页面时改写旧设置。 */
    private static int trigger(boolean spaceVoice, boolean voiceShortcut) {
        return spaceVoice || voiceShortcut ? TRIGGER_COMBINED : TRIGGER_NONE;
    }

    /** 未设置时按普通话显示（键盘的默认识别语言）；其他平台写入的语言原样显示。 */
    private static String languageLabel(String value) {
        return VoiceLanguageCatalog.title(value);
    }
}
