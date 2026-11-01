package app.msime.android.home;

import android.content.Context;
import android.text.InputType;
import android.widget.EditText;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AiProviderSettings;
import app.msime.android.CandidateTranslationPolicy;
import app.msime.android.CandidateTranslationRoute;
import app.msime.android.JsonPolicy;
import app.msime.android.MSIMEInputService;
import app.msime.android.NativeClient;
import app.msime.android.TranslationServiceSettingsPolicy;
import app.msime.android.TranslationServiceSettingsPolicy.Choice;
import app.msime.android.core.InputViewValuePolicy;
import java.util.function.Function;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 翻译服务：候选词在离线释义之外，用哪个在线服务补译文。可选不使用、水杉账号、小牛翻译、腾讯云机器翻译、自定义（DeepLX 或 DeepL API）、AI 辅助（复用「AI 润色与回复」页的配置）。
 *
 * <p>全部写在与桌面、iOS 相同的共享偏好里：`niutrans`、`custom_translation`、`tencent_tmt` 三段和 `translation_account`、`translation_ai`。选服务时三个 `enabled` 只开一个，腾讯没选时也显式写 false（它的默认值是开着），`translation_account` 只在选水杉账号时写 true、`translation_ai` 只在选 AI 辅助时写 true，否则删掉，与共享设置页的 `setTranslationProvider` 和 iOS 的 `select` 相同；别的服务的凭据保留，切回来不用重填。凭据与 AI 辅助的令牌一样只存在本机的偏好里，不随设置同步上传；密钥行只显示「已填写」或「未填写」。
 *
 * <p>每个字段点开是一个输入框，按共享偏好的校验规则（{@link TranslationServiceSettingsPolicy}）当场说明哪里不行，不合规的值不会写入。键盘怎么选路见 {@link CandidateTranslationPolicy#route}：所选服务凭据不全时不联网，也不改用别的服务。
 */
public final class TranslationServicePage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("服务", "配置");
    }

    /** 读一次页面要的东西：偏好、接口地址下的提示，自定义服务的共享描述器能不能构造（DeepL 地址没填密钥时不能），以及 AI 辅助的配置能不能用、用的是哪家服务商的哪个模型。 */
    private record State(JSONObject preferences, String endpointIssue, boolean customBuildable, boolean aiReady,
            String aiSummary) {}

    /** 测试连接的结果；message 是共享层给的一句话，不含凭据和服务的回复。 */

    /**
     * 本页刚选过的服务。腾讯的 `enabled` 默认开着，凭据填好之前从偏好里分不出「用户选了腾讯」，所以刚选腾讯时按这里显示，好让用户接着填凭据；离开再回来时按偏好显示。
     */
    @Nullable private Choice chosen;
    /** 选了自己的服务（小牛、腾讯、自定义、AI）时，候选词怎样发出去、怎样计费。 */
    private static final String USER_SERVICE_NOTE = "选择自己的服务后，键盘把当前一页的中文候选（最多 10 个，每个不超过 256 字）直接发给该服务，不经过水杉服务器。小牛、自定义和 AI 每个词单独请求一次；设置了第二目标语言时，每种语言各请求一次，都会消耗你在该服务的额度。离线释义查得到的语言仍优先显示离线释义。";
    private boolean testing;

    @Override protected void onContentDestroyed() {
        // ViewResultDelivery drops a probe result that outlives this view; release its temporary
        // lock so a rebuilt page can start another test, and let the next read decide the service.
        testing = false;
        chosen = null;
    }

    @Override protected void reload() {
        HostTask.run(this, TranslationServicePage::read, this::render);
    }

    @Nullable private static State read(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        if (preferences == null) return null;
        String endpoint = InputViewValuePolicy.text(preferences.optJSONObject("custom_translation"), "endpoint");
        String issue = "";
        boolean buildable = false;
        try {
            if (!endpoint.isEmpty()) {
                JSONObject value = envelope(NativeClient.translationEndpointIssue(endpoint));
                issue = value == null ? "" : JsonPolicy.strictStringOrEmpty(value.opt("issue"));
            }
            JSONObject custom = preferences.optJSONObject("custom_translation");
            JSONObject probe = new JSONObject()
                .put("config", new JSONObject().put("enabled", true).put("endpoint", endpoint)
                    .put("api_key", InputViewValuePolicy.text(custom, "api_key")))
                .put("text", "你好").put("source_language", "zh").put("target_language", "en");
            buildable = !endpoint.isEmpty()
                && envelope(NativeClient.customTranslationHttpRequest(probe.toString())) != null;
        } catch (JSONException | RuntimeException | LinkageError error) {
            // 共享库没加载上或回答读不懂：不显示提示，按凭据不完整处理。
        }
        JSONObject ai = preferences.optJSONObject("ai_assistant");
        return new State(preferences, issue, buildable, aiReady(ai), aiSummary(ai));
    }

    /** AI 辅助的配置能不能用：只拿 `ai_assistant` 一段、当作选了 AI 辅助交给键盘的同一个选路，避免页面和键盘各算一套。 */
    private static boolean aiReady(@Nullable JSONObject ai) {
        if (ai == null) return false;
        try {
            JSONObject probe = new JSONObject().put("translation_ai", true).put("ai_assistant", ai)
                .put("tencent_tmt", new JSONObject().put("enabled", false));
            return MSIMEInputService.candidateTranslationRouteFrom(probe).provider() == CandidateTranslationRoute.Provider.AI;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return false;
        }
    }

    /** 「当前服务商与模型」一行的值，与共享设置页的写法相同；没有配置时为空。 */
    private static String aiSummary(@Nullable JSONObject ai) {
        String provider = InputViewValuePolicy.text(ai, "provider");
        if (provider.isEmpty()) return "";
        String title = provider;
        for (AiProviderSettings.Provider entry : AiProviderSettings.catalog()) {
            if (entry.id().equals(provider)) title = entry.title();
        }
        String model = InputViewValuePolicy.text(ai, "model");
        return title + " · " + (model.isEmpty() ? "未配置" : model);
    }

    private void render(@Nullable State state) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        if (state == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        JSONObject preferences = state.preferences();
        JSONObject niutrans = section(preferences, "niutrans");
        JSONObject custom = section(preferences, "custom_translation");
        JSONObject tencent = section(preferences, "tencent_tmt");
        boolean tencentEnabled = JsonPolicy.strictBoolean(tencent.opt("enabled"), true);
        Choice stored = TranslationServiceSettingsPolicy.selected(
            JsonPolicy.strictBoolean(preferences.opt("translation_account"), false),
            JsonPolicy.strictBoolean(niutrans.opt("enabled"), false),
            JsonPolicy.strictBoolean(custom.opt("enabled"), false),
            tencentEnabled, InputViewValuePolicy.text(tencent, "secret_id"), InputViewValuePolicy.text(tencent, "secret_key"),
            JsonPolicy.strictBoolean(preferences.opt("translation_ai"), false));
        Choice selected = chosen == Choice.TENCENT && stored == Choice.NONE && tencentEnabled ? Choice.TENCENT : stored;
        boolean translations = JsonPolicy.strictBoolean(preferences.opt("candidate_translations"), true);

        // 这一组没有组标题：「翻译服务」这一行的「?」除了说明这项做什么，也写明所选服务怎样处理候选词。
        String routeNote = switch (selected) {
            case NONE -> "键盘不会把候选词发给任何在线服务；离线释义照常显示。";
            case ACCOUNT -> "候选词会发送到水杉服务器（api.msime.app）翻译，首次使用会自动创建匿名账号。";
            default -> selected.userService() ? USER_SERVICE_NOTE : null;
        };
        GroupCard service = GroupCard.add(sectionColumn(target, "服务"), null);
        service.nav("翻译服务", null, selected.title(), () -> pick(selected))
            .help("离线释义查不到的词，用这里选的服务补上译文。" + (routeNote == null ? "" : "\n\n" + routeNote));
        if (!translations) service.note("「候选词翻译」已关闭，键盘不会联网翻译。打开它在「输入 → 翻译」里。");

        switch (selected) {
            case NONE:
            case ACCOUNT:
                break;
            case NIUTRANS:
                renderNiuTrans(target, niutrans);
                break;
            case TENCENT:
                renderTencent(target, tencent);
                break;
            case CUSTOM:
                renderCustom(target, custom, state.endpointIssue());
                break;
            case AI:
                renderAi(target, state.aiReady(), state.aiSummary());
                break;
        }
        if (selected.userService()) renderTest(target, selected, niutrans, custom, tencent, state.customBuildable());
    }

    private void renderNiuTrans(LinearLayout target, JSONObject niutrans) {
        GroupCard group = GroupCard.add(sectionColumn(target, "配置"), "小牛翻译");
        String appId = InputViewValuePolicy.text(niutrans, "app_id");
        group.nav("App ID", null, appId.isEmpty() ? "未填写" : HomeTextPolicy.shortLabel(appId), () -> edit("App ID", null, appId,
            InputType.TYPE_CLASS_TEXT, TranslationServiceSettingsPolicy::niutransProblem, "niutrans", "app_id"));
        group.nav("API Key", null, filled(InputViewValuePolicy.text(niutrans, "apikey")), () -> edit("API Key", "小牛翻译控制台里的 API Key，保存在本机", "",
            secretInput(), TranslationServiceSettingsPolicy::niutransProblem, "niutrans", "apikey"));
        group.help("在小牛翻译开放平台创建应用后取得 App ID 和 API Key。");
    }

    private void renderTencent(LinearLayout target, JSONObject tencent) {
        GroupCard group = GroupCard.add(sectionColumn(target, "配置"), "腾讯云机器翻译");
        String region = InputViewValuePolicy.text(tencent, "region");
        group.nav("SecretId", null, filled(InputViewValuePolicy.text(tencent, "secret_id")), () -> edit("SecretId", "腾讯云访问密钥的 SecretId，保存在本机", "",
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS,
            TranslationServiceSettingsPolicy::secretIdProblem, "tencent_tmt", "secret_id"));
        group.nav("SecretKey", null, filled(InputViewValuePolicy.text(tencent, "secret_key")), () -> edit("SecretKey", "腾讯云访问密钥的 SecretKey，保存在本机", "",
            secretInput(), TranslationServiceSettingsPolicy::secretProblem, "tencent_tmt", "secret_key"));
        group.nav("地域", null, region.isEmpty() ? CandidateTranslationRoute.DEFAULT_TENCENT_REGION : region,
            () -> edit("地域", "留空使用 " + CandidateTranslationRoute.DEFAULT_TENCENT_REGION, region,
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS,
                TranslationServiceSettingsPolicy::regionProblem, "tencent_tmt", "region"));
        group.help("建议为输入法单独创建只授权机器翻译的子账号密钥。腾讯一次最多翻译 9 个候选词。");
    }

    /** AI 辅助没有自己的凭据：显示复用的是哪家服务商的哪个模型，配置不全时说明去哪里补；文案与共享设置页的「复用 AI 辅助配置」相同。 */
    private void renderAi(LinearLayout target, boolean ready, String summary) {
        GroupCard group = GroupCard.add(sectionColumn(target, "配置"), "复用 AI 辅助配置");
        group.nav("当前服务商与模型", ready ? null : "请先在「AI 润色与回复」页面填写接口地址、Token 和模型",
            summary.isEmpty() ? "未配置" : summary, () -> SettingsNavigator.open(requireContext(), PageId.AI_SETTINGS, null));
        if (!ready) group.note("AI 配置不完整，键盘暂不联网翻译，也不会改用其他服务。");
        group.help("复用「AI 润色与回复」页当前服务商的接口地址、Token 与模型，无需在此重复填写；AI 模型逐条生成释义，成本与延迟高于专用翻译服务。不需要打开「启用 AI 入口」。");
    }

    private void renderCustom(LinearLayout target, JSONObject custom, String endpointIssue) {
        GroupCard group = GroupCard.add(sectionColumn(target, "配置"), "自定义接口");
        String endpoint = InputViewValuePolicy.text(custom, "endpoint");
        group.nav("接口地址", endpointIssue.isEmpty() ? null : endpointIssue,
            endpoint.isEmpty() ? "未设置" : HomeTextPolicy.shortLabel(endpoint), () -> edit("接口地址",
                "兼容 DeepLX 的 POST 接口，例如 http://127.0.0.1:1188/translate；或 DeepL 官方 API https://api-free.deepl.com/v2/translate",
                endpoint, InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI,
                TranslationServicePage::endpointProblem, "custom_translation", "endpoint"))
            .help("https 地址，或本机回环的 http 地址");
        group.nav("API Key", null, filled(InputViewValuePolicy.text(custom, "api_key")),
            () -> edit("API Key", "DeepL 填 Auth Key（Free 密钥以 :fx 结尾），保存在本机；留空表示不需要", "",
                secretInput(), TranslationServiceSettingsPolicy::secretProblem, "custom_translation", "api_key")).help("DeepL 官方 API 必填；DeepLX 没设密钥时留空");
        group.help("DeepL 官方地址按 DeepL 的格式请求，并按密钥自动选 Free 或 Pro 主机；其他地址按 DeepLX 的格式请求。使用 DeepL 时候选词会发送到 DeepL，按 DeepL 的数据政策处理。远程服务必须用 https，http 只能指向本机回环地址，不能是局域网地址。");
    }

    /** 凭据是否齐全的提示和「测试连接」：测试用的是已经保存的值，与键盘实际用的一致。 */
    private void renderTest(LinearLayout target, Choice selected, JSONObject niutrans, JSONObject custom,
            JSONObject tencent, boolean customBuildable) {
        GroupCard group = GroupCard.add(sectionColumn(target, "配置"), null);
        CandidateTranslationRoute route = CandidateTranslationPolicy.route(true, false,
            selected == Choice.NIUTRANS, InputViewValuePolicy.text(niutrans, "app_id"), InputViewValuePolicy.text(niutrans, "apikey"),
            selected == Choice.CUSTOM, InputViewValuePolicy.text(custom, "endpoint"), InputViewValuePolicy.text(custom, "api_key"),
            selected == Choice.TENCENT, InputViewValuePolicy.text(tencent, "secret_id"), InputViewValuePolicy.text(tencent, "secret_key"), InputViewValuePolicy.text(tencent, "region"));
        boolean complete = route.userService() && (selected != Choice.CUSTOM || customBuildable);
        if (!complete) group.note("凭据不完整，键盘暂不联网翻译，也不会改用其他服务。");
        group.button("测试连接", null, "测试", () -> {
            if (testing) return;
            testing = true;
            MsToast.show(requireContext(), "正在测试…");
            JSONObject config = testConfig(selected, niutrans, custom, tencent);
            HostTask.runNetwork(this, context -> ServiceCredentialTest.api(selected.testService(), config), result -> {
                testing = false;
                if (!isAdded()) return;
                MessageDialog.show(requireContext(), result.ok() ? "测试成功" : "测试未通过", result.message());
            });
        }).help("用已保存的设置翻译一次「测试」，最长约 15 秒");
    }

    private static JSONObject testConfig(Choice selected, JSONObject niutrans, JSONObject custom, JSONObject tencent) {
        JSONObject config = new JSONObject();
        try {
            switch (selected) {
                case NIUTRANS:
                    config.put("app_id", InputViewValuePolicy.text(niutrans, "app_id")).put("apikey", InputViewValuePolicy.text(niutrans, "apikey"));
                    break;
                case TENCENT:
                    config.put("secret_id", InputViewValuePolicy.text(tencent, "secret_id")).put("secret_key", InputViewValuePolicy.text(tencent, "secret_key"))
                        .put("region", InputViewValuePolicy.text(tencent, "region"));
                    break;
                case CUSTOM:
                    config.put("endpoint", InputViewValuePolicy.text(custom, "endpoint")).put("api_key", InputViewValuePolicy.text(custom, "api_key"));
                    break;
                default:
                    break;
            }
        } catch (JSONException impossible) {
            // 只放字符串，put 不会失败；真失败时用空配置，共享层回答「请填写有效的…」。
        }
        return config;
    }

    private void pick(Choice selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "翻译服务", null);
        for (Choice choice : Choice.values()) {
            sheet.option(choice.title(), choice == selected, () -> select(choice));
        }
        sheet.show();
    }

    /** 只开一个服务：三个 `enabled` 互斥，腾讯没选时显式写 false，`translation_account` 只在选水杉账号时写 true，`translation_ai` 只在选 AI 辅助时写 true。 */
    private void select(Choice choice) {
        chosen = choice;
        KeyboardSheets.save(this, preferences -> {
            KeyboardSheets.child(preferences, "niutrans").put("enabled", choice == Choice.NIUTRANS);
            KeyboardSheets.child(preferences, "custom_translation").put("enabled", choice == Choice.CUSTOM);
            JSONObject tencent = KeyboardSheets.child(preferences, "tencent_tmt");
            tencent.put("enabled", choice == Choice.TENCENT);
            if (InputViewValuePolicy.text(tencent, "region").isEmpty())
                tencent.put("region", CandidateTranslationRoute.DEFAULT_TENCENT_REGION);
            if (choice == Choice.ACCOUNT) preferences.put("translation_account", true);
            else preferences.remove("translation_account");
            if (choice == Choice.AI) preferences.put("translation_ai", true);
            else preferences.remove("translation_ai");
        }, this::reload, this::reload);
    }

    /**
     * 编辑一个字段：输入框里的值去掉首尾空白后按 `problem` 检查，不合规时在输入框下说明原因并禁用「保存」。密钥字段打开时是空的（不回显已存的密钥），留空保存表示清掉。
     */
    private void edit(String title, @Nullable String message, String current, int inputType,
            Function<String, String> problem, String section, String key) {
        InputDialog dialog = new InputDialog(requireContext(), title, message);
        EditText field = dialog.addField(title, current, inputType);
        dialog.setValidator(values -> {
            String reason = problem.apply(values.get(0));
            field.setError(reason);
            return reason == null;
        });
        dialog.setPrimary("保存", values -> KeyboardSheets.save(this, preferences -> {
            String value = values.get(0);
            if (problem.apply(value) != null) throw new JSONException(key + " is not valid");
            if ("region".equals(key) && value.isEmpty()) value = CandidateTranslationRoute.DEFAULT_TENCENT_REGION;
            KeyboardSheets.child(preferences, section).put(key, value);
        }, this::reload, this::reload));
        dialog.show();
    }

    private static int secretInput() {
        return InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD;
    }

    private static JSONObject section(JSONObject preferences, String key) {
        JSONObject value = preferences.optJSONObject(key);
        return value == null ? new JSONObject() : value;
    }

    /** 接口地址能不能存，问共享层：它与保存偏好时的校验是同一条规则，带下划线的主机名、国际化域名这类 https 地址不会被页面误拒。 */
    @Nullable private static String endpointProblem(String value) {
        Boolean accepted = null;
        String issue = "";
        if (!value.isEmpty()) {
            try {
                JSONObject result = envelope(NativeClient.translationEndpointIssue(value));
                Object raw = result == null ? null : result.opt("accepted");
                if (raw instanceof Boolean) {
                    accepted = (Boolean) raw;
                    issue = JsonPolicy.strictStringOrEmpty(result.opt("issue"));
                }
            } catch (JSONException | RuntimeException | LinkageError error) {
                // 共享库没加载上或回答读不懂：退回本地规则。
            }
        }
        return TranslationServiceSettingsPolicy.endpointProblem(value, accepted, issue);
    }

    @Nullable private static JSONObject envelope(String response) throws JSONException {
        if (response == null || response.isEmpty()) return null;
        JSONObject root = new JSONObject(response);
        return JsonPolicy.objectValue(root);
    }

    private static String filled(String value) {
        return value.isEmpty() ? "未填写" : "已填写";
    }

}
