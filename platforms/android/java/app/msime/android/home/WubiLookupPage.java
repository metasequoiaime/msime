package app.msime.android.home;

import app.msime.android.FirstRunPreparation;

import android.content.Context;
import android.os.Bundle;
import android.text.InputFilter;
import android.text.InputType;
import android.view.View;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.lifecycle.ViewModel;
import androidx.lifecycle.ViewModelProvider;
import app.msime.android.DimensionPolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.KeyboardScheme;
import app.msime.android.LayoutPolicy;
import app.msime.android.NativeClient;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.WubiCodeLookupPolicy;
import app.msime.android.WubiLookupRequestState;
import app.msime.android.core.InputViewValuePolicy;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 五笔查码（#6789）：输入或粘贴汉字，列出每个字（以及整段词）的 86 和 98 五笔编码。
 *
 * <p>与共享设置页的「五笔查码」一组同样的文字和排法（规则在 {@link WubiCodeLookupPolicy}）。安卓的产品是这个原生宿主，没有 WebView，共享设置页那一组到不了用户手里，所以在这里原生画一遍。查码走词库请求 `wubi_codes`（{@link NativeClient#personalDictionaryRequest}）：host-api 在入口直接回答，不进个人词库队列，在设置应用进程里只读打开 `msime-wubi.db`，不需要键盘在跑，也不挡键盘的词库维护。
 */
public final class WubiLookupPage extends DetailPage {
    /** 一次查码的结果：失败时 {@link #failed} 为真，其余字段为空。 */
    private record Result(boolean failed, List<WubiCodeLookupPolicy.Codes> rows, boolean truncated, int listed,
            String profile) {
        static Result failure() { return new Result(true, List.of(), false, 0, WubiCodeLookupPolicy.WUBI_86); }
    }

    /** 输入框里的文字和上一次的结果放在 ViewModel 里：旋转、换深浅模式重建 Fragment 时不丢。 */
    public static final class State extends ViewModel {
        final WubiLookupRequestState<Result> lookup = new WubiLookupRequestState<>();
    }

    private State state;
    @Nullable private TextView search;
    @Nullable private GroupCard results;
    /** 等共享准备完成合表与回放，避免 SQLite 忙时把暂时读不到误当成没有编码。 */
    private final FirstRunPreparation.Listener preparationListener = value -> refreshButton();

    @Override public void onStart() {
        super.onStart();
        FirstRunPreparation.observe(preparationListener);
    }

    @Override public void onStop() {
        FirstRunPreparation.stopObserving(preparationListener);
        super.onStop();
    }

    @Override public void onCreate(@Nullable Bundle saved) {
        super.onCreate(saved);
        state = new ViewModelProvider(this).get(State.class);
    }

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        Context context = requireContext();
        GroupCard input = GroupCard.add(column, "查汉字的五笔编码");
        EditText field = ViewPolicy.styledInput(context, Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
        field.setHint("例如：水杉输入法");
        field.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        field.setFilters(new InputFilter[] {new InputFilter.LengthFilter(WubiCodeLookupPolicy.MAX_LENGTH)});
        ViewPolicy.setTopStart(field);
        ViewPolicy.setMinLines(field, 2);
        ViewPolicy.clearBackground(field);
        field.setHintTextColor(ThemeColorPolicy.subText(context));
        ViewPolicy.setSymmetricPaddingDp(context, field, 16, 14);
        field.setContentDescription("要查五笔编码的文字");
        field.setText(state.lookup.text());
        input.card().addView(field, LayoutPolicy.matchWidthWrapParams());
        input.help(WubiCodeLookupPolicy.DESCRIPTION);

        TextView button = ViewPolicy.textButton(context, "查码", 16, 600, ThemeColorPolicy.onAccent(context), null,
            Ui.ACTION_BUTTON_MIN_HEIGHT, this::lookup);
        LinearLayout.LayoutParams buttonParams = LayoutPolicy.matchWidthWrapParams();
        buttonParams.topMargin = DimensionPolicy.pixels(context, Ui.GROUP_GAP);
        column.addView(button, buttonParams);
        search = button;

        results = GroupCard.add(column, null).withDividers(Ui.ROW_PADDING_H);
        ViewPolicy.afterTextChanged(field, text -> {
            if (state.lookup.edit(text.toString())) {
                refreshButton();
                renderResults();
            }
        });
        refreshButton();
        renderResults();
    }

    @Override public void onDestroyView() {
        state.lookup.viewDestroyed();
        search = null;
        results = null;
        super.onDestroyView();
    }

    private void refreshButton() {
        Context context = getContext();
        TextView button = search;
        if (context == null || button == null) return;
        boolean prepared = FirstRunPreparation.state() == FirstRunPreparation.State.READY;
        boolean ready = prepared && !state.lookup.busy() && WubiCodeLookupPolicy.searchable(state.lookup.text());
        button.setText(!prepared ? "词库准备中…" : state.lookup.busy() ? "正在查码…" : "查码");
        ViewPolicy.setEnabled(button, ready);
        ViewPolicy.setTextColor(button, ready ? ThemeColorPolicy.onAccent(context) : ThemeColorPolicy.subText(context));
        int fill = ready ? ThemeColorPolicy.accent(context)
            : ThemeColorPolicy.color(context, com.google.android.material.R.attr.colorSurfaceContainerHighest);
        ViewPolicy.setBackground(button, ViewPolicy.ripple(context, fill, DimensionPolicy.pixels(context, Ui.GROUP_RADIUS)));
    }

    /** 同一段输入只发一个请求；编辑或视图销毁后，旧回答不能写回。 */
    private void lookup() {
        if (FirstRunPreparation.state() != FirstRunPreparation.State.READY) return;
        long requestGeneration = state.lookup.begin();
        if (requestGeneration == 0) return;
        String text = state.lookup.text();
        refreshButton();
        HostTask.run(this, context -> query(context, text), result -> {
            if (!state.lookup.complete(requestGeneration, result == null ? Result.failure() : result)) return;
            refreshButton();
            renderResults();
        });
    }

    /** 设置应用进程里问一次词库请求 `wubi_codes`；任何一步失败都算查码失败。 */
    private static Result query(Context context, String text) {
        try {
            JSONObject preferences = KeyboardSheets.preferences(context);
            String profile = KeyboardScheme.normalizedWubiProfile(preferences == null ? KeyboardScheme.WUBI_86
                : InputViewValuePolicy.textOr(preferences, "wubi_profile", KeyboardScheme.WUBI_86));
            JSONObject request = new JSONObject()
                .put("options", new JSONObject(CloudSync.hostOptions(context)))
                .put("action", new JSONObject().put("operation", "wubi_codes").put("text", text));
            JSONObject value = CloudSync.nativeValue(NativeClient.personalDictionaryRequest(request.toString()));
            JSONArray characters = value.optJSONArray("characters");
            if (characters == null) return Result.failure();
            List<WubiCodeLookupPolicy.Codes> listed = new ArrayList<>(characters.length());
            for (int index = 0; index < characters.length(); index++) {
                WubiCodeLookupPolicy.Codes codes = codes(characters.optJSONObject(index));
                if (codes == null) return Result.failure();
                listed.add(codes);
            }
            WubiCodeLookupPolicy.Codes phrase = value.isNull("phrase") ? null : codes(value.optJSONObject("phrase"));
            return new Result(false, WubiCodeLookupPolicy.rows(phrase, listed),
                JsonPolicy.strictTrue(value.opt("truncated")), listed.size(), profile);
        } catch (IOException | JSONException | RuntimeException error) {
            return Result.failure();
        }
    }

    /** `{text, wubi86, wubi98}`；形状不对时为 null。 */
    @Nullable private static WubiCodeLookupPolicy.Codes codes(@Nullable JSONObject item) {
        if (item == null) return null;
        String text = JsonPolicy.strictString(item.opt("text"));
        List<String> wubi86 = strings(item.optJSONArray("wubi86"));
        List<String> wubi98 = strings(item.optJSONArray("wubi98"));
        if (text == null || wubi86 == null || wubi98 == null) return null;
        return new WubiCodeLookupPolicy.Codes(text, wubi86, wubi98);
    }

    @Nullable private static List<String> strings(@Nullable JSONArray array) {
        if (array == null) return null;
        List<String> values = new ArrayList<>(array.length());
        for (int index = 0; index < array.length(); index++) {
            String value = JsonPolicy.strictString(array.opt(index));
            if (value == null) return null;
            values.add(value);
        }
        return values;
    }

    private void renderResults() {
        GroupCard card = results;
        if (card == null) return;
        card.card().removeAllViews();
        Result result = state.lookup.result();
        ViewPolicy.setVisible(card.view(), result != null);
        if (result == null) return;
        if (result.failed()) {
            card.note(WubiCodeLookupPolicy.FAILED);
            return;
        }
        if (result.rows().isEmpty()) {
            card.note(WubiCodeLookupPolicy.NOTHING);
            return;
        }
        for (WubiCodeLookupPolicy.Codes codes : result.rows()) {
            GroupCard.Row row = card.value(codes.text(), WubiCodeLookupPolicy.columns(codes, result.profile()), null);
            View view = row.view();
            view.setContentDescription(WubiCodeLookupPolicy.description(codes, result.profile()));
        }
        if (result.truncated()) card.note(WubiCodeLookupPolicy.truncated(result.listed()));
    }
}
