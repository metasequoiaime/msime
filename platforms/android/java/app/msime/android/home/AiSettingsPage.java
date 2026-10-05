package app.msime.android.home;

import android.os.Bundle;
import android.text.InputType;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.AiPolishConfiguration;
import app.msime.android.TextPolicy;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * AI 润色与回复：启用开关、端点、模型、凭据和润色提示词，全部存在共享偏好的 `ai_assistant` 里，由你自己的服务端提供。
 *
 * <p>与原来键盘页的底部面板写法相同：启用时端点必须是 https 地址（键盘会拒绝别的地址，这里也不写进去）；凭据按端点的来源（origin）分别存放在 `ai_assistant.tokens` 里，换端点不会把凭据带到另一个主机，留空凭据只删掉这个来源的那一条；提示词写进 `prompt_id` 选中的那个槽位，也就是键盘读的那个。每一项点开是一个输入框，确认后立即保存。
 */
public final class AiSettingsPage extends DetailPage {
    private static final int MAX_ENDPOINT_BYTES = 2048;

    @Nullable private LinearLayout column;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        reload();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        super.onDestroyView();
    }

    private void reload() {
        HostTask.run(this, KeyboardSheets::preferences, this::render);
    }

    private void render(@Nullable JSONObject preferences) {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        if (preferences == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        JSONObject ai = preferences.optJSONObject("ai_assistant");
        if (ai == null) ai = new JSONObject();
        boolean enabled = ai.optBoolean("enabled", false);
        String endpoint = ai.optString("endpoint", "");
        String model = ai.optString("model", "");
        String origin = originOf(endpoint);
        JSONObject tokens = ai.optJSONObject("tokens");
        boolean hasToken = tokens != null && !origin.isEmpty() && !tokens.optString(origin, "").isEmpty();
        String promptKey = AiPolishConfiguration.promptSlotKey(ai.optString("prompt_id", ""));
        String prompt = ai.optString(promptKey, "");

        GroupCard service = GroupCard.add(target, "服务");
        GroupCard.Row[] enabledRow = new GroupCard.Row[1];
        enabledRow[0] = service.toggle("启用 AI 入口", "在键盘上显示 AI 回复与润色", enabled, checked -> {
            if (checked && !validEndpoint(endpoint)) {
                enabledRow[0].setChecked(false);
                MsToast.show(requireContext(), "请先填写一个 https 端点");
                return;
            }
            saveAi(next -> next.put("enabled", checked));
        });
        service.nav("端点 URL", "必须是 https 地址", endpoint.isEmpty() ? "未设置" : shorten(endpoint),
            () -> editEndpoint(endpoint, enabled));
        service.nav("模型", null, model.isEmpty() ? "未设置" : model, () -> editText("模型", null, model,
            InputType.TYPE_CLASS_TEXT, value -> saveAi(next -> next.put("model", value))));
        service.nav("凭据", origin.isEmpty() ? "先填写端点" : "只用于 " + origin, hasToken ? "已设置" : "未设置",
            origin.isEmpty() ? null : () -> editToken(origin));

        GroupCard polish = GroupCard.add(target, "润色");
        polish.nav("润色提示词", null, prompt.isEmpty() ? "默认" : shorten(prompt), () -> editText("润色提示词",
            "留空使用默认提示词", prompt, InputType.TYPE_CLASS_TEXT,
            value -> saveAi(next -> next.put(promptKey, value))));
        polish.footer("凭据只保存在本机，按端点的来源分别存放。留空凭据表示该来源不需要凭据，不会清掉其他来源已存的凭据。");
    }

    private void editEndpoint(String current, boolean enabled) {
        InputDialog dialog = new InputDialog(requireContext(), "端点 URL", "例如 https://api.example.com/v1/chat/completions");
        dialog.addField("https://", current, InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
        // 启用时端点不能为空；关闭时可以清空。
        dialog.setValidator(values -> values.get(0).isEmpty() ? !enabled : validEndpoint(values.get(0)));
        dialog.setPrimary("保存", values -> saveAi(next -> next.put("endpoint", values.get(0))));
        dialog.show();
    }

    private void editToken(String origin) {
        InputDialog dialog = new InputDialog(requireContext(), "凭据", "只用于 " + origin + "，保存在本机");
        dialog.addField("留空表示不需要凭据", "", InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        dialog.setValidator(values -> true);
        dialog.setPrimary("保存", values -> saveAi(next -> {
            String endpointOrigin = originOf(next.optString("endpoint", ""));
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
            if (ai.optBoolean("enabled", false) && !validEndpoint(ai.optString("endpoint", "")))
                throw new JSONException("endpoint must be https");
        }, this::reload, this::reload);
    }

    private static boolean validEndpoint(String endpoint) {
        return TextPolicy.validAuthority(endpoint, "https://", MAX_ENDPOINT_BYTES);
    }

    private static String originOf(String endpoint) {
        if (endpoint.isEmpty()) return "";
        try {
            return AiPolishConfiguration.credentialOrigin(endpoint);
        } catch (RuntimeException error) {
            return "";
        }
    }

    private static String shorten(String value) {
        String single = value.replace('\n', ' ');
        return single.length() <= 24 ? single : single.substring(0, 23) + "…";
    }
}
