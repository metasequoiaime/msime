package app.msime.android;

import android.content.res.ColorStateList;
import android.graphics.Typeface;
import android.graphics.drawable.StateListDrawable;
import android.os.Build;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.Menu;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.TextView;
import app.msime.android.CandidateTranslationPolicy;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 候选按钮的构建与样式、展开的候选面板和候选长按菜单；从 MSIMEInputService 原样搬出，状态仍在服务里。 */
final class ImeCandidates {
    private final MSIMEInputService s;

    ImeCandidates(MSIMEInputService s) {
        this.s = s;
    }

    void styleCandidateButton(Button button) {
        StateListDrawable states = new StateListDrawable();
        states.addState(new int[] {android.R.attr.state_selected},
            s.imeStyler.candidateDrawable(s.candidateAppearance.selected()));
        states.addState(new int[] {android.R.attr.state_pressed},
            s.imeStyler.candidateDrawable(s.candidateAppearance.hover()));
        states.addState(new int[] {android.R.attr.state_focused},
            s.imeStyler.candidateDrawable(s.candidateAppearance.hover()));
        states.addState(new int[] {android.R.attr.state_hovered},
            s.imeStyler.candidateDrawable(s.candidateAppearance.hover()));
        states.addState(new int[0], s.imeStyler.candidateDrawable(s.candidateAppearance.surface()));
        button.setBackground(states);
        button.setTextColor(new ColorStateList(
            new int[][] {{android.R.attr.state_selected}, {}},
            new int[] {s.candidateAppearance.textFor(true), s.candidateAppearance.text()}));
        // The design marks the highlighted candidate with bold accent text and no fill.
        button.setTypeface(s.imeStyler.candidateTypeface(), button.isSelected() ? Typeface.BOLD : Typeface.NORMAL);
        button.setElevation(0);
    }

    boolean showCandidateMenu(Button button, int slot, JSONObject id, String text) {
        if (!s.candidateGlossInsertionEnabled() && !s.candidateManagementEnabled()) return false;
        return showCandidateMenu(button, slot, id, text, s.visibleCandidate(slot), false);
    }

    boolean showCandidateMenu(Button button, int slot, JSONObject id, String text,
                                      JSONObject candidate, boolean expanded) {
        if (!s.candidateGlossInsertionEnabled() && !s.candidateManagementEnabled()) return false;
        PopupMenu popup = new PopupMenu(s, button);
        String translation = candidate == null || candidate.isNull("translation")
            ? "" : candidate.optString("translation", "");
        java.util.List<String> glosses = s.candidateGlossInsertionEnabled()
            ? CandidateTranslationPolicy.insertionGlosses(translation) : java.util.List.of();
        for (int index = 0; index < glosses.size(); index++)
            popup.getMenu().add(Menu.NONE, 2000 + index, Menu.NONE, glosses.get(index));
        boolean management = s.candidateManagementEnabled();
        if (glosses.isEmpty() && !management) return false;
        if (management) {
            for (CandidateManagementAction action : CandidateManagementAction.values()) {
                popup.getMenu().add(Menu.NONE, action.menuItemId(), action.ordinal(), action.title());
            }
        }
        popup.setOnMenuItemClickListener(item -> {
            s.imeKeyFeedback.playFeedback(button);
            int glossIndex = item.getItemId() - 2000;
            if (glossIndex >= 0 && glossIndex < glosses.size()) {
                if (expanded) s.insertExpandedCandidateGloss(candidate, id, text, glosses.get(glossIndex));
                else s.insertCandidateGloss(slot, id, text, glosses.get(glossIndex));
                return true;
            }
            CandidateManagementAction action;
            try {
                action = CandidateManagementAction.fromMenuItemId(item.getItemId());
            } catch (IllegalArgumentException error) {
                return false;
            }
            if (action.confirmationRequired()) {
                s.confirmCandidateRemoval(id, text);
                return true;
            }
            s.editCandidate(id, action);
            return true;
        });
        popup.show();
        return true;
    }

    Button makeCandidateButton(int slot) {
        // Apple uses the same press-feedback button for candidate chips as for keys. Android's
        // HorizontalScrollView cancels the child on a drag, so the button keeps immediate tap
        // feedback without changing the existing scroll-versus-select boundary.
        Button button = new KeyboardPressButton(s);
        button.setAllCaps(false);
        button.setOnClickListener(ignored -> s.selectVisibleCandidate(button, slot));
        button.setOnLongClickListener(ignored -> {
            JSONObject current = s.visibleCandidate(slot);
            JSONObject id = current == null ? null : current.optJSONObject("id");
            if (id == null || (!s.candidateManagementEnabled() && !s.candidateGlossInsertionEnabled())) return false;
            String text = s.chineseOutput(current.optString("text"), s.view);
            return showCandidateMenu(button, slot, id, text);
        });
        return button;
    }

    Button expandedCandidateButton(JSONObject candidate) {
        JSONObject id = candidate.optJSONObject("id");
        Button button = new KeyboardPressButton(s);
        String text = s.chineseOutput(candidate.optString("text"), s.view);
        boolean highlighted = candidate.optBoolean("highlighted");
        String typed = s.candidatePanelSnapshot == null ? ""
            : s.candidatePanelSnapshot.optString("preedit", "");
        String annotation = s.candidateAnnotation(candidate, typed);
        button.setAllCaps(false);
        button.setText(s.candidateLabel("", text, annotation, highlighted));
        int labelLines = MSIMEInputService.candidateLabelLines(annotation);
        button.setMinLines(labelLines);
        button.setMaxLines(labelLines);
        s.configureCandidateTextLayout(button, labelLines);
        button.setTextSize(TypedValue.COMPLEX_UNIT_SP, s.candidateFontSize);
        button.setSelected(highlighted);
        styleCandidateButton(button);
        long index = id == null ? -1
            : CandidateGlossPolicy.strictOr(id.opt("index"), -1);
        button.setContentDescription(index < 0 ? "候选" : "候选 " + (index + 1) + "："
            + text + s.candidateAccessibilitySuffix(candidate, typed));
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(highlighted ? "已选中" : "未选中");
        if (id == null || index < 0) {
            button.setEnabled(false);
        } else {
            button.setOnClickListener(ignored -> {
                s.imeKeyFeedback.playFeedback(button);
                if (s.session == 0
                        || CandidateGlossPolicy.strictOr(id.opt("session"), Long.MIN_VALUE)
                            != s.session) return;
                s.candidatePanelOpen = false;
                s.candidatePanelSnapshot = null;
                try {
                    s.apply(NativeClient.selectAnyCandidate(
                        s.session, MSIMEInputService.strictCandidateLong(id, "generation"),
                        MSIMEInputService.strictCandidateLong(id, "index")));
                } catch (JSONException | LinkageError error) { s.fail(); }
            });
            button.setOnLongClickListener(ignored -> {
                if (!s.candidateManagementEnabled() && !s.candidateGlossInsertionEnabled()) return false;
                return showCandidateMenu(button, -1, id, text, candidate, true);
            });
        }
        return button;
    }

    void renderExpandedCandidates() {
        if (s.expandedCandidates == null || s.expandedCandidateScroll == null) return;
        s.expandedCandidates.removeAllViews();
        if (!s.candidatePanelOpen || s.view == null || s.candidatePanelSnapshot == null
                || CandidateGlossPolicy.strictOr(s.candidatePanelSnapshot.opt("session"), Long.MIN_VALUE)
                    != s.session
                || !MSIMEInputService.sameCandidateVersion(s.candidatePanelSnapshot, s.view)) {
            s.candidatePanelOpen = false;
            s.candidatePanelSnapshot = null;
            s.expandedCandidates.setVisibility(View.GONE);
            s.expandedCandidateScroll.setVisibility(View.GONE);
            return;
        }
        s.expandedCandidateScroll.setVisibility(View.VISIBLE);
        s.expandedCandidates.setVisibility(View.VISIBLE);
        LinearLayout header = new LinearLayout(s);
        header.setGravity(Gravity.CENTER_VERTICAL);
        TextView composition = new TextView(s);
        String reading = s.candidatePanelSnapshot.optString("reading", "");
        String compositionText = reading.isEmpty()
            ? s.candidatePanelSnapshot.optString("preedit", "") : reading;
        composition.setText(compositionText);
        composition.setTextSize(TypedValue.COMPLEX_UNIT_SP, s.candidatePreeditFontSize);
        composition.setContentDescription("当前组合文本：" + compositionText);
        header.addView(composition, new LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        JSONArray entries = s.candidatePanelSnapshot.optJSONArray("candidates");
        int count = entries == null ? 0 : entries.length();
        TextView countView = new TextView(s);
        countView.setText(count + " 个候选");
        countView.setContentDescription(count + " 个候选");
        header.addView(countView, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        Button close = new Button(s);
        close.setAllCaps(false);
        close.setText("收起");
        close.setContentDescription("收起候选面板");
        close.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(close);
            s.closeCandidatePanel();
        });
        header.addView(close, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.WRAP_CONTENT,
            LinearLayout.LayoutParams.WRAP_CONTENT));
        s.expandedCandidates.addView(header);
        CandidateWrapLayout list = new CandidateWrapLayout(s, s.pixels(8));
        list.setPadding(0, s.pixels(8), 0, 0);
        list.setContentDescription("完整候选列表");
        if (entries != null) {
            for (int index = 0; index < entries.length(); index++) {
                JSONObject candidate = entries.optJSONObject(index);
                if (candidate == null) continue;
                Button button = expandedCandidateButton(candidate);
                list.addView(button, new android.view.ViewGroup.LayoutParams(
                    android.view.ViewGroup.LayoutParams.WRAP_CONTENT,
                    android.view.ViewGroup.LayoutParams.WRAP_CONTENT));
            }
        }
        s.expandedCandidates.addView(list);
    }
}
