package app.msime.android.home;

import android.content.Context;
import android.text.InputType;
import android.widget.EditText;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AiEndpointPolicy;
import app.msime.android.AiPolishConfiguration;
import app.msime.android.AiProviderSettings;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.TextPolicy;
import app.msime.android.core.InputViewValuePolicy;
import java.util.List;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * AI 润色与回复：启用开关、端点、模型、凭据和润色提示词，全部存在共享偏好的 `ai_assistant` 里，由你自己的服务端提供。
 *
 * <p>最下面是「社区回复模板」：社区里别人分享的 AI 回复模板列前几个，点开看详情、可以举报，末尾「浏览更多回复模板」进完整列表（{@link CommunityFragment}）。回复模板手机上还不能添加进键盘的回复面板，这里和完整列表都只能浏览。
 *
 * <p>与原来键盘页的底部面板写法相同：启用时端点必须是 https 地址，或指向本机、局域网的 http 地址（规则见 `AiEndpointPolicy`，键盘会拒绝别的地址，这里也不写进去）；凭据按端点的来源（origin）分别存放在 `ai_assistant.tokens` 里，换端点不会把凭据带到另一个主机，留空凭据只删掉这个来源的那一条；提示词写进 `prompt_id` 选中的那个槽位，也就是键盘读的那个。服务商与模型使用选择面板，地址、凭据与提示词使用输入框，确认后立即保存。
 */
public final class AiSettingsPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("服务", "润色", "社区");
    }

    /** 「社区回复模板」一组最多列几个，多的在完整列表里。 */
    private static final int REPLY_LIMIT = 5;

    /** 社区回复模板的第一页；null 是还没读到或读失败，失败原因在 {@link #repliesFailure}。 */
    @Nullable private List<CommunityCatalog.Item> replies;
    @Nullable private String repliesFailure;
    private boolean repliesRequested;

    private record State(JSONObject preferences, List<AiProviderSettings.Provider> catalog) {}

    /** 比较请求归属时才用到凭据；禁止默认 record 的 toString 输出它。 */
    private record ModelRequest(String provider, String endpoint, String token) {
        @Override public String toString() { return "AI model catalog request"; }
    }

    private List<String> fetchedModels = List.of();
    @Nullable private ModelRequest fetchedFor;
    private boolean modelsBusy;
    @Nullable private String modelsStatus;
    private long editGeneration;

    @Override protected void reload() {
        HostTask.run(this, context -> new State(KeyboardSheets.preferences(context), AiProviderSettings.catalog()), this::render);
        // 社区目录每次打开这一页只读一次，读到之后再按刚读的偏好整页重画。
        if (!repliesRequested) {
            repliesRequested = true;
            HostTask.runNetwork(this,
                context -> new CommunityCatalog(context).list(CommunityRequest.Kind.REPLY, "", 0, null),
                page -> {
                    CommunityCatalog.Discovery result = CommunityCatalog.discovery(page, REPLY_LIMIT);
                    replies = result.items();
                    repliesFailure = result.failure();
                    reload();
                });
        }
    }

    @Override protected void onContentDestroyed() {
        editGeneration++;
        modelsBusy = false;
        fetchedFor = null;
        fetchedModels = List.of();
    }

    private void render(@Nullable State state) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        JSONObject preferences = state == null ? null : state.preferences();
        if (preferences == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        JSONObject ai = preferences.optJSONObject("ai_assistant");
        if (ai == null) ai = new JSONObject();
        List<AiProviderSettings.Provider> catalog = state.catalog();
        String provider = InputViewValuePolicy.textOr(ai, "provider", "");
        boolean enabled = ai.optBoolean("enabled", false);
        String endpoint = InputViewValuePolicy.textOr(ai, "endpoint", "");
        String model = InputViewValuePolicy.textOr(ai, "model", "");
        String origin = originOf(endpoint);
        JSONObject tokens = ai.optJSONObject("tokens");
        boolean hasToken = tokens != null && !origin.isEmpty() && !tokens.optString(origin, "").isEmpty();
        String promptKey = AiPolishConfiguration.promptSlotKey(
            InputViewValuePolicy.textOr(ai, "prompt_id", ""));
        String prompt = ai.optString(promptKey, "");

        GroupCard service = GroupCard.add(sectionColumn(target, "服务"), "服务");
        AiProviderSettings.Provider selected = null;
        for (AiProviderSettings.Provider entry : catalog) if (entry.id().equals(provider)) selected = entry;
        // 目录不可用、端点有问题、获取模型的结果是状态，留在副标题；平时的用法说明收进「?」。
        service.nav("服务商", catalog.isEmpty() ? "服务目录暂不可用，仍可手动填写地址与模型" : null,
            selected == null ? (provider.isEmpty() ? "未设置" : provider) : selected.title(),
            catalog.isEmpty() ? null : () -> pickProvider(provider, catalog))
            .help("选择后自动填写预设地址和模型");
        String problem = endpointProblem(endpoint);
        service.nav("端点 URL", problem,
            endpoint.isEmpty() ? "未设置" : HomeTextPolicy.shortLabel(endpoint),
            () -> editEndpoint(endpoint, enabled))
            .help("https 地址，或本机、局域网的 http 地址");
        service.nav("凭据", !origin.isEmpty() ? "只用于 " + origin
                : problem != null ? "公网端点必须用 https，凭据不会明文发出" : "先填写端点", hasToken ? "已设置" : "未设置",
            origin.isEmpty() ? null : () -> editToken(origin));
        java.util.LinkedHashSet<String> choices = new java.util.LinkedHashSet<>();
        if (!model.isEmpty()) choices.add(model);
        if (selected != null) choices.addAll(selected.models());
        ModelRequest request = modelRequest(ai, catalog);
        if (!request.equals(fetchedFor)) {
            fetchedFor = request;
            fetchedModels = List.of();
            modelsStatus = null;
        }
        choices.addAll(fetchedModels);
        service.nav("模型", null, model.isEmpty() ? "未设置" : model,
            () -> pickModel(model, List.copyOf(choices))).help("从列表选择，也可以自定义模型名称");
        service.nav("获取模型列表", modelsStatus,
            modelsBusy ? "获取中…" : null,
            modelsBusy || origin.isEmpty() ? null : () -> fetchModels(request))
            .help("从当前服务读取可用模型；不会自动联网");
        if (selected != null && !selected.documentation().isEmpty()) {
            String documentation = selected.documentation();
            service.nav("AI 接入说明与 API Key", null, null,
                () -> {
                    if (!LinkActions.open(requireContext(), documentation))
                        MsToast.show(requireContext(), "没有可以打开链接的应用");
                }).help("查看所选服务的官方接入说明");
        }
        GroupCard.Row[] enabledRow = new GroupCard.Row[1];
        enabledRow[0] = service.toggle("启用 AI 入口", null, enabled, checked -> {
            if (checked && !AiEndpointPolicy.allowed(endpoint)) {
                enabledRow[0].setChecked(false);
                String reason = endpointProblem(endpoint);
                MsToast.show(requireContext(), reason != null ? reason : "请先填写一个 https 端点，或本机、局域网的 http 端点");
                return;
            }
            saveAi(next -> next.put("enabled", checked));
        }).help("在键盘上显示 AI 回复与润色");
        service.help("AI 设计皮肤使用水杉账号服务，请到外观中的「AI 设计皮肤」操作。");
        service.nav("AI 对话", null, null,
            () -> ((HomeActivity) requireActivity()).openChatSecondary(false)).help("在独立页面里对话，同时可以试用键盘");

        GroupCard polish = GroupCard.add(sectionColumn(target, "润色"), "润色");
        polish.nav("润色提示词", null, prompt.isEmpty() ? "默认" : HomeTextPolicy.shortLabel(prompt), () -> editText("润色提示词",
            "留空使用默认提示词", prompt, InputType.TYPE_CLASS_TEXT,
            value -> saveAi(next -> next.put(promptKey, value))));
        polish.help("凭据只保存在本机，按端点的来源分别存放。留空凭据表示该来源不需要凭据，不会清掉其他来源已存的凭据。");

        renderReplies(target);
    }

    /** 社区里的 AI 回复模板：前几个，点开看详情、举报和屏蔽作者，末尾进完整列表。 */
    private void renderReplies(LinearLayout target) {
        Context context = requireContext();
        GroupCard group = GroupCard.add(sectionColumn(target, "社区"), "社区回复模板").withDividers(58);
        List<CommunityCatalog.Item> items = replies;
        if (items == null) {
            group.note(repliesFailure == null ? "正在读取社区回复模板…" : repliesFailure);
        } else if (items.isEmpty()) {
            group.note("社区里还没有回复模板。");
        } else {
            for (CommunityCatalog.Item item : items) {
                group.addView(KeyboardSheets.badgeNavRow(context, TextPolicy.initial(item.name(), "回"), item.name(),
                    item.author().isEmpty() ? null : "@" + item.author(), null, () -> showReply(item)));
            }
        }
        group.nav("浏览更多回复模板", null, null,
            () -> CommunityFragment.browse(context, CommunityRequest.Kind.REPLY)).help("搜索社区里的全部回复模板");
        // 这类模板是什么收进组标题的「?」；手机上还不能用是宿主限制，留在原处。
        group.help("别人分享的 AI 回复提示词。");
        group.footer("手机上暂时只能浏览，还不能添加到键盘的回复面板。");
    }

    /** 回复模板的详情：和完整列表里点开的是同一个面板，没有「添加」，可以举报和屏蔽作者。 */
    private void showReply(CommunityCatalog.Item item) {
        // 回复模板没有皮肤，面板不画键盘预览，布局传标准样式即可。
        CommunitySkinSheet.show(requireContext(), item, KeyLayoutStylesPolicy.Preview.STANDARD, "", null,
            () -> CommunityReportDialog.show(this, item),
            item.owned() ? null : () -> CommunityBlockDialog.show(this, item.author(),
                CommunityRequest.reportKind(item.kind()), item.id(), this::reloadReplies),
            null);
    }

    /** 屏蔽作者之后重读社区回复模板：服务端已不再列出这位作者的模板；这位作者的皮肤也不再列出，所以一并刷新键盘皮肤面板用的目录缓存。 */
    private void reloadReplies() {
        CommunitySkinCatalogue.refresh(this, true, ignored -> { });
        repliesRequested = false;
        replies = null;
        repliesFailure = null;
        reload();
    }

    private void pickProvider(String current, List<AiProviderSettings.Provider> catalog) {
        OptionSheet sheet = new OptionSheet(requireContext(), "服务商", "切换保留手填地址、模型和各服务已存的凭据");
        for (AiProviderSettings.Provider entry : catalog) {
            if (entry.desktopOnly() && !entry.id().equals(current)) continue;
            sheet.option(entry.title(), entry.id().equals(current),
                () -> saveAi(ai -> AiProviderSettings.switchProvider(ai, entry.id())));
        }
        sheet.show();
    }

    private void pickModel(String current, List<String> models) {
        OptionSheet sheet = new OptionSheet(requireContext(), "模型", "预设仅供参考，实际可用模型由服务和账号决定");
        for (String model : models) sheet.option(model, model.equals(current),
            () -> saveAi(ai -> ai.put("model", model)));
        sheet.option("自定义…", false, () -> editText("模型", null, current, InputType.TYPE_CLASS_TEXT,
            value -> saveAi(ai -> ai.put("model", value))));
        sheet.show();
    }

    private static ModelRequest modelRequest(JSONObject ai, List<AiProviderSettings.Provider> catalog) {
        String endpoint = InputViewValuePolicy.textOr(ai, "endpoint", "");
        String provider = InputViewValuePolicy.textOr(ai, "provider", "");
        JSONObject tokens = ai.optJSONObject("tokens");
        String origin = originOf(endpoint);
        String token = tokens == null || origin.isEmpty() ? "" : InputViewValuePolicy.textOr(tokens, origin, "");
        // 服务目录不可用时不猜鉴权规则；不把旧 Token 交给不鉴权的服务。
        boolean usesToken = false;
        for (AiProviderSettings.Provider entry : catalog) if (entry.id().equals(provider)) usesToken = entry.usesToken();
        return new ModelRequest(provider, endpoint, usesToken ? token : "");
    }

    private void fetchModels(ModelRequest request) {
        long generation = editGeneration;
        modelsBusy = true;
        modelsStatus = null;
        reload();
        HostTask.runNetwork(this, context -> {
            try {
                return AiProviderSettings.fetchModels(request.endpoint(), request.token());
            } catch (JSONException | IllegalArgumentException | LinkageError error) {
                return List.<String>of();
            }
        }, result -> HostTask.run(this, KeyboardSheets::preferences, preferences -> {
            modelsBusy = false;
            JSONObject latest = preferences == null ? null : preferences.optJSONObject("ai_assistant");
            if (generation != editGeneration || latest == null || !request.equals(modelRequest(latest, AiProviderSettings.catalog()))) {
                reload();
                return;
            }
            fetchedFor = request;
            fetchedModels = result == null ? List.of() : result;
            modelsStatus = fetchedModels.isEmpty() ? "获取失败，可使用预设或手动填写模型名称" : "已获取 " + fetchedModels.size() + " 个模型";
            reload();
        }));
    }

    private void editEndpoint(String current, boolean enabled) {
        InputDialog dialog = new InputDialog(requireContext(), "端点 URL",
            "例如 https://api.example.com/v1/chat/completions；本机或局域网的模型服务（如 LM Studio）也可以用 http://192.168.1.20:1234/v1/chat/completions");
        EditText field = dialog.addField("https:// 或局域网 http://", current,
            InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        // 启用时端点不能为空；关闭时可以清空。公网 http 地址当场说明为什么不行，而不是只把「保存」变灰。
        dialog.setValidator(values -> {
            String value = values.get(0);
            String reason = value.isEmpty() ? null : endpointProblem(value);
            field.setError(reason);
            return value.isEmpty() ? !enabled : AiEndpointPolicy.allowed(value);
        });
        dialog.setPrimary("保存", values -> saveAi(next -> next.put("endpoint", values.get(0))));
        dialog.show();
    }

    private void editToken(String origin) {
        InputDialog dialog = new InputDialog(requireContext(), "凭据", "只用于 " + origin + "，保存在本机");
        dialog.addField("留空表示不需要凭据", "", InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        dialog.setValidator(values -> true);
        dialog.setPrimary("保存", values -> saveAi(next -> {
            String endpointOrigin = originOf(InputViewValuePolicy.textOr(next, "endpoint", ""));
            // 键盘在对话框打开期间改了端点时不写：凭据只属于确认时所见的那个来源。
            if (!origin.equals(endpointOrigin)) throw new JSONException("endpoint changed");
            JSONObject tokens = KeyboardSheets.child(next, "tokens");
            String token = values.get(0);
            if (token.isEmpty()) tokens.remove(origin); else tokens.put(origin, token);
        }));
        dialog.show();
    }

    private interface TextAction {
        void accept(String value);
    }

    private void editText(String title, @Nullable String message, String current, int inputType, TextAction save) {
        InputDialog dialog = new InputDialog(requireContext(), title, message);
        dialog.addField(title, current, inputType);
        dialog.setValidator(values -> true);
        dialog.setPrimary("保存", values -> save.accept(values.get(0)));
        dialog.show();
    }

    private interface AiEdit {
        void apply(JSONObject ai) throws JSONException;
    }

    /** 改 `ai_assistant` 的一处；改完后启用状态下端点不合法就整次不写，键盘继续用磁盘上原来的设置。 */
    private void saveAi(AiEdit edit) {
        editGeneration++;
        KeyboardSheets.save(this, preferences -> {
            JSONObject ai = KeyboardSheets.child(preferences, "ai_assistant");
            edit.apply(ai);
            if (ai.optBoolean("enabled", false)
                    && !AiEndpointPolicy.allowed(InputViewValuePolicy.textOr(ai, "endpoint", "")))
                throw new JSONException("endpoint is not allowed");
        }, this::reload, this::reload);
    }

    /** 公网 http 地址被拒绝的原因；其他情况（包括地址可用或格式不对）为 null。 */
    @Nullable private static String endpointProblem(String endpoint) {
        return !endpoint.isEmpty() && AiEndpointPolicy.check(endpoint) == AiEndpointPolicy.Result.CLEARTEXT_PUBLIC
            ? AiEndpointPolicy.CLEARTEXT_REASON : null;
    }

    private static String originOf(String endpoint) {
        if (endpoint.isEmpty()) return "";
        try {
            return AiPolishConfiguration.credentialOrigin(endpoint);
        } catch (RuntimeException error) {
            return "";
        }
    }

}
