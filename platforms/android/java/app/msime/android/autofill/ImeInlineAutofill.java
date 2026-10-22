package app.msime.android;

import android.annotation.TargetApi;
import android.graphics.Color;
import android.os.Bundle;
import android.util.Size;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.InlineSuggestion;
import android.view.inputmethod.InlineSuggestionsRequest;
import android.view.inputmethod.InlineSuggestionsResponse;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.inline.InlineContentView;
import android.widget.inline.InlinePresentationSpec;
import java.util.ArrayList;
import java.util.List;

/** 只挂载系统隔离的填充视图，不把凭据转成文本、候选或剪贴板条目。 */
@TargetApi(30)
final class ImeInlineAutofill {
    private static final int MAX_SUGGESTIONS = 5;
    private final MSIMEInputService s;
    private final InlineAutofillGeneration generation = new InlineAutofillGeneration();
    private final List<InlineContentView> views = new ArrayList<>();
    private HorizontalScrollView row;
    private LinearLayout content;

    ImeInlineAutofill(MSIMEInputService service) { s = service; }

    InlineSuggestionsRequest request(Bundle rendererExtras) {
        Bundle style = InlineAutofillStyles.create(rendererExtras,
            Color.parseColor(s.skin.keyForeground()), Color.parseColor(s.skin.keyBackground()));
        if (style == null) return null;
        int height = s.pixels(36);
        int width = Math.max(s.pixels(48), s.getResources().getDisplayMetrics().widthPixels - s.pixels(8));
        InlinePresentationSpec spec = new InlinePresentationSpec.Builder(
            new Size(s.pixels(40), height), new Size(width, height)).setStyle(style).build();
        return new InlineSuggestionsRequest.Builder(List.of(spec))
            .setMaxSuggestionCount(MAX_SUGGESTIONS).build();
    }

    void startInput() { clearViews(); generation.startInput(); }
    void finishInput() { clearViews(); generation.finishInput(); }
    void showInput() { generation.showInput(); }

    private void clearViews() {
        if (content != null) content.removeAllViews();
        views.clear();
        if (row != null) row.setVisibility(View.GONE);
    }

    boolean response(InlineSuggestionsResponse response) {
        clearViews();
        long expected = generation.nextResponse();
        if (!generation.accepts(expected)) return false;
        List<InlineSuggestion> suggestions = response.getInlineSuggestions();
        int count = Math.min(MAX_SUGGESTIONS, suggestions.size());
        for (int index = 0; index < count; index++) views.add(null);
        boolean requested = false;
        for (int index = 0; index < count; index++) {
            final int position = index;
            try {
                suggestions.get(index).inflate(s,
                    new Size(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT),
                    s.getMainExecutor(), view -> {
                        if (!generation.accepts(expected) || view == null) return;
                        views.set(position, view);
                        if (content != null) {
                            int slot = 0;
                            for (int before = 0; before < position; before++) if (views.get(before) != null) slot++;
                            content.addView(view, slot, layoutParams(view));
                        }
                        if (s.keyboardRoot != null) s.render();
                    });
                requested = true;
            } catch (RuntimeException unavailable) {
                // 服务或渲染器失效时跳过该项，不记录响应内容。
            }
        }
        if (s.keyboardRoot != null) s.render();
        return requested;
    }

    void buildRow(LinearLayout region) {
        if (content != null) content.removeAllViews();
        row = new HorizontalScrollView(s);
        row.setHorizontalScrollBarEnabled(false);
        row.setContentDescription("自动填充");
        row.setPadding(s.pixels(4), s.pixels(2), s.pixels(4), s.pixels(2));
        content = new LinearLayout(s);
        content.setGravity(Gravity.CENTER_VERTICAL);
        row.addView(content, new ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.MATCH_PARENT));
        region.addView(row, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
            s.pixels(KeyboardGeometry.DESIGN_TOOLBAR_ROW_HEIGHT_DP)));
        row.setVisibility(View.GONE);
        mountViews();
    }

    private void mountViews() {
        if (content == null) return;
        content.removeAllViews();
        for (InlineContentView view : views) {
            if (view == null) continue;
            if (view.getParent() instanceof ViewGroup old) old.removeView(view);
            content.addView(view, layoutParams(view));
        }
    }

    private LinearLayout.LayoutParams layoutParams(InlineContentView view) {
        // `inflate` 回调已经写入远程界面的实际尺寸；改成 `WRAP_CONTENT` 会让 `SurfaceView` 宽度归零。
        ViewGroup.LayoutParams actual = view.getLayoutParams();
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(actual.width, actual.height);
        params.setMarginEnd(s.pixels(4));
        return params;
    }

    boolean update(boolean allowed) {
        boolean visible = allowed && content != null && content.getChildCount() != 0;
        if (row != null) row.setVisibility(visible ? View.VISIBLE : View.GONE);
        return visible;
    }

    void setHeight(int height) {
        if (row != null) ViewPolicy.setFixedHeight(row, height);
    }
}
