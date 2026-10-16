package app.msime.android.home;

import android.content.Context;
import android.text.InputType;
import android.widget.EditText;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AiEndpointPolicy;
import app.msime.android.AiPolishConfiguration;
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
 * <p>与原来键盘页的底部面板写法相同：启用时端点必须是 https 地址，或指向本机、局域网的 http 地址（规则见 `AiEndpointPolicy`，键盘会拒绝别的地址，这里也不写进去）；凭据按端点的来源（origin）分别存放在 `ai_assistant.tokens` 里，换端点不会把凭据带到另一个主机，留空凭据只删掉这个来源的那一条；提示词写进 `prompt_id` 选中的那个槽位，也就是键盘读的那个。每一项点开是一个输入框，确认后立即保存。
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

    @Override protected void reload() {
        HostTask.run(this, KeyboardSheets::preferences, this::render);
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

    private void render(@Nullable JSONObject preferences) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        if (preferences == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        JSONObject ai = preferences.optJSONObject("ai_assistant");
        if (ai == null) ai = new JSONObject();
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
        GroupCard.Row[] enabledRow = new GroupCard.Row[1];
        enabledRow[0] = service.toggle("启用 AI 入口", "在键盘上显示 AI 回复与润色", enabled, checked -> {
            if (checked && !AiEndpointPolicy.allowed(endpoint)) {
                enabledRow[0].setChecked(false);
                String reason = endpointProblem(endpoint);
                MsToast.show(requireContext(), reason != null ? reason : "请先填写一个 https 端点，或本机、局域网的 http 端点");
                return;
            }
            saveAi(next -> next.put("enabled", checked));
        });
        String problem = endpointProblem(endpoint);
        service.nav("端点 URL", problem != null ? problem : "https 地址，或本机、局域网的 http 地址",
            endpoint.isEmpty() ? "未设置" : HomeTextPolicy.shortLabel(endpoint),
            () -> editEndpoint(endpoint, enabled));
        service.nav("模型", null, model.isEmpty() ? "未设置" : model, () -> editText("模型", null, model,
            InputType.TYPE_CLASS_TEXT, value -> saveAi(next -> next.put("model", value))));
        service.nav("凭据", !origin.isEmpty() ? "只用于 " + origin
                : problem != null ? "公网端点必须用 https，凭据不会明文发出" : "先填写端点", hasToken ? "已设置" : "未设置",
            origin.isEmpty() ? null : () -> editToken(origin));
        service.nav("AI 对话", "在独立页面里对话，同时可以试用键盘", null,
            () -> ((HomeActivity) requireActivity()).openChatSecondary(false));

        GroupCard polish = GroupCard.add(sectionColumn(target, "润色"), "润色");
        polish.nav("润色提示词", null, prompt.isEmpty() ? "默认" : HomeTextPolicy.shortLabel(prompt), () -> editText("润色提示词",
            "留空使用默认提示词", prompt, InputType.TYPE_CLASS_TEXT,
            value -> saveAi(next -> next.put(promptKey, value))));
        polish.footer("凭据只保存在本机，按端点的来源分别存放。留空凭据表示该来源不需要凭据，不会清掉其他来源已存的凭据。");

        renderReplies(target);
    }

    /** 社区里的 AI 回复模板：前几个，点开看详情和举报，末尾进完整列表。 */
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
        group.nav("浏览更多回复模板", "搜索社区里的全部回复模板", null,
            () -> CommunityFragment.browse(context, CommunityRequest.Kind.REPLY));
        group.footer("别人分享的 AI 回复提示词。手机上暂时只能浏览，还不能添加到键盘的回复面板。");
    }

    /** 回复模板的详情：和完整列表里点开的是同一个面板，没有「添加」，可以举报。 */
    private void showReply(CommunityCatalog.Item item) {
        CommunitySkinSheet.show(requireContext(), item, false, "", null,
            () -> CommunityReportDialog.show(this, item), null);
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
