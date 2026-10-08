package app.msime.android;

import android.graphics.Typeface;
import android.os.Build;
import android.util.TypedValue;
import android.view.Menu;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import app.msime.android.CandidateTranslationPolicy;
import app.msime.android.core.InputViewValuePolicy;
import app.msime.android.core.InputViewValuePolicy;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 候选按钮的构建与样式、展开的候选面板和候选长按菜单；从 MSIMEInputService 原样搬出，状态仍在服务里。 */
final class ImeCandidates {
    private final MSIMEInputService s;
    private String styleCacheKey;
    private int selectedBackground;
    private int selectedText;
    private int keyBackground;
    private int keyForeground;
    private int accentSoft;
    private int accentText;
    private Typeface candidateTypeface;

    ImeCandidates(MSIMEInputService s) {
        this.s = s;
    }

    /** Resolve skin strings once per appearance; candidate rows can contain many buttons. */
    private void ensureStyleCache() {
        String key = s.skin.key() + ":" + s.candidateAppearance.key();
        if (key.equals(styleCacheKey)) return;
        styleCacheKey = key;
        selectedBackground = android.graphics.Color.parseColor(s.skin.candidateSelectedBackground());
        selectedText = android.graphics.Color.parseColor(s.skin.candidateSelectedForeground());
        keyBackground = android.graphics.Color.parseColor(s.skin.keyBackground());
        keyForeground = android.graphics.Color.parseColor(s.skin.keyForeground());
        accentSoft = android.graphics.Color.parseColor(s.skin.accentSoft());
        accentText = android.graphics.Color.parseColor(s.skin.accentText());
        candidateTypeface = s.imeStyler.candidateTypeface();
    }

    /** 首选候选 chip：字母键的底（kb.key）、圆角 9，皮肤强调色 600 字重；其余候选不画底。 */
    private android.graphics.drawable.GradientDrawable chip(int color, int radiusDp) {
        return DrawablePolicy.rounded(color, s.pixels(radiusDp));
    }

    void styleCandidateButton(Button button) {
        ensureStyleCache();
        if (expandedCells.contains(button)) {
            styleExpandedCell(button);
            return;
        }
        android.graphics.drawable.Drawable pressed =
            s.imeStyler.candidateDrawable(s.candidateAppearance.hover());
        android.graphics.drawable.Drawable focused =
            s.imeStyler.candidateDrawable(s.candidateAppearance.hover());
        android.graphics.drawable.Drawable hovered =
            s.imeStyler.candidateDrawable(s.candidateAppearance.hover());
        android.graphics.drawable.StateListDrawable states = DrawablePolicy.stateList(
            new int[][] {
                {android.R.attr.state_selected}, {android.R.attr.state_pressed},
                {android.R.attr.state_focused}, {android.R.attr.state_hovered}, new int[0]
            },
            chip(selectedBackground, 9), pressed, focused, hovered,
            chip(android.graphics.Color.TRANSPARENT, 9));
        ViewPolicy.setBackground(button, states);
        ViewPolicy.setTextColor(button, ColorPolicy.stateList(
            new int[][] {{android.R.attr.state_selected}, {}},
            new int[] {selectedText, keyForeground}));
        applyCandidateTypeface(button);
        ViewPolicy.setMinimumWidth(button, s.pixels(30));
        ViewPolicy.clearMinimumHeight(button);
        KeyboardGeometry.setHorizontalPaddingDp(button, s, 11);
        ViewPolicy.setLineSpacing(button, 0, 1.0f);
        ViewPolicy.clearFontPadding(button);
        ViewPolicy.clearElevation(button);
    }

    private void applyCandidateTypeface(Button button) {
        button.setTypeface(candidateTypeface,
            button.isSelected() ? Typeface.BOLD : Typeface.NORMAL);
    }

    /** 展开网格里的单元：样式通道重走整棵树时按网格单元上色，而不是按候选条的 chip。 */
    private final java.util.Set<Button> expandedCells =
        java.util.Collections.newSetFromMap(new java.util.WeakHashMap<>());

    /** 展开网格的单元：44 dp 高、圆角 8，平时 kb.key 底，当前高亮的那个 accentSoft 底 + 强调色字。 */
    private void styleExpandedCell(Button button) {
        android.graphics.drawable.StateListDrawable states = DrawablePolicy.stateList(
            new int[][] {
                {android.R.attr.state_selected}, {android.R.attr.state_pressed}, new int[0]
            },
            chip(accentSoft, 8), chip(s.candidateAppearance.hover(), 8), chip(keyBackground, 8));
        ViewPolicy.setBackground(button, states);
        ViewPolicy.setTextColor(button, ColorPolicy.stateList(
            new int[][] {{android.R.attr.state_selected}, {}},
            new int[] {accentText, keyForeground}));
        applyCandidateTypeface(button);
        ViewPolicy.clearElevation(button);
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
            ? "" : JsonPolicy.strictStringOrEmpty(candidate.opt("translation"));
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
        Button button = candidateButton();
        ViewPolicy.bindClick(button, () -> s.selectVisibleCandidate(button, slot));
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
        Button button = candidateButton();
        String text = s.chineseOutput(candidate.optString("text"), s.view);
        boolean highlighted = InputViewValuePolicy.booleanValue(candidate, "highlighted", false);
        String typed = s.candidatePanelSnapshot == null ? ""
            : s.candidatePanelSnapshot.optString("preedit", "");
        String annotation = s.candidateAnnotation(candidate, typed);
        button.setText(s.candidateLabel("", text, annotation, highlighted));
        int labelLines = MSIMEInputService.candidateLabelLines(annotation);
        ViewPolicy.setFixedLines(button, labelLines);
        s.configureCandidateTextLayout(button, labelLines);
        KeyboardGeometry.setKeyTextSize(button, 17);
        ViewPolicy.setSelected(button, highlighted);
        expandedCells.add(button);
        ViewPolicy.setMinimumSize(button, s.pixels(64), s.pixels(44));
        KeyboardGeometry.setHorizontalPaddingDp(button, s, 10);
        // The completed keyboard tree is styled once by MSIMEInputService.render().
        // Styling here would be repeated immediately after this button is attached.
        long index = id == null ? -1
            : CandidateGlossPolicy.strictOr(id.opt("index"), -1);
        button.setContentDescription(index < 0 ? "候选" : "候选 " + (index + 1) + "："
            + text + s.candidateAccessibilitySuffix(candidate, typed));
        if (Build.VERSION.SDK_INT >= 30)
            button.setStateDescription(highlighted ? "已选中" : "未选中");
        if (id == null || index < 0) {
            ViewPolicy.setEnabled(button, false);
        } else {
            ViewPolicy.bindClick(button, () -> {
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

    private KeyboardPressButton expandedActionButton(String label, float sizeSp,
            String description, Runnable action) {
        KeyboardPressButton button = candidateButton();
        button.setKeyboardRole(KeyboardKeyRole.ACCENT);
        ViewPolicy.setTextSizeLabel(button, label, sizeSp);
        KeyboardGeometry.setKeyTextSize(button, sizeSp);
        button.setContentDescription(description);
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            action.run();
        });
        return button;
    }

    private KeyboardPressButton candidateButton() {
        KeyboardPressButton button = ViewPolicy.newPressButton(s);
        return button;
    }

    void renderExpandedCandidates() {
        if (s.expandedCandidates == null || s.expandedCandidateScroll == null) return;
        if (!s.candidatePanelOpen) {
            // The normal keyboard render reaches this method on every keystroke. The panel is
            // normally already hidden by closeCandidatePanel(); avoid traversing and clearing an
            // empty subtree until the next open actually needs to rebuild it.
            if (s.expandedCandidates.getVisibility() != View.GONE) {
                ViewPolicy.hide(s.expandedCandidates);
                ViewPolicy.hide(s.expandedCandidateScroll);
            }
            // 选了候选等路径只把 candidatePanelOpen 置为 false，九键三栏面板在这里跟着收起并清掉筛选。
            s.imeNineKeyPanel.dismiss();
            return;
        }
        // 全拼九键的三栏面板：选拼音、⌫、筛选换了一代候选时不收起，重新取完整候选再画；组字结束时 eligible 为假，落到下面按代次收起。
        if (s.imeNineKeyPanel.eligible() && s.imeNineKeyPanel.refreshSnapshot()) {
            if (s.expandedCandidates.getVisibility() != View.GONE) {
                s.expandedCandidates.removeAllViews();
                ViewPolicy.hide(s.expandedCandidates);
                ViewPolicy.hide(s.expandedCandidateScroll);
            }
            s.imeNineKeyPanel.render();
            return;
        }
        s.imeNineKeyPanel.dismiss();
        s.expandedCandidates.removeAllViews();
        if (s.view == null || s.candidatePanelSnapshot == null
                || CandidateGlossPolicy.strictOr(s.candidatePanelSnapshot.opt("session"), Long.MIN_VALUE)
                    != s.session
                || !MSIMEInputService.sameCandidateVersion(s.candidatePanelSnapshot, s.view)) {
            s.closeCandidatePanel();
            return;
        }
        ViewPolicy.show(s.expandedCandidateScroll);
        ViewPolicy.show(s.expandedCandidates);
        KeyboardGeometry.setSymmetricPaddingDp(s.expandedCandidates, s, 8, 6);
        JSONArray entries = s.candidatePanelSnapshot.optJSONArray("candidates");
        int count = entries == null ? 0 : entries.length();
        String reading = InputViewValuePolicy.textOr(s.candidatePanelSnapshot, "reading", "");
        String compositionText = reading.isEmpty()
            ? s.candidatePanelSnapshot.optString("preedit", "") : reading;
        // 设计的网格不画标题；组合文本和候选总数留在网格的描述里给读屏。
        CandidateWrapLayout list = new CandidateWrapLayout(s, s.pixels(6));
        list.setContentDescription("完整候选列表；" + compositionText + "；" + count + " 个候选");
        if (entries != null) {
            for (int index = 0; index < entries.length(); index++) {
                JSONObject candidate = entries.optJSONObject(index);
                if (candidate == null) continue;
                Button button = expandedCandidateButton(candidate);
                list.addView(button, new android.view.ViewGroup.LayoutParams(
                    android.view.ViewGroup.LayoutParams.WRAP_CONTENT, s.pixels(44)));
            }
        }
        s.expandedCandidates.addView(list, KeyboardGeometry.weightedWidthParams(1));
        // 底部 返回 + ⌫，各 40 dp 高、功能键底色。
        LinearLayout footer = KeyboardGeometry.row(s);
        KeyboardPressButton close = expandedActionButton("返回", 15, "收起候选面板", () -> {
            s.closeCandidatePanel();
            s.render();
        });
        KeyboardPressButton delete = expandedActionButton("⌫", 16, "候选面板 删除", () -> {
            s.closeCandidatePanel();
            s.deleteFromHandwriting();
        });
        LinearLayout.LayoutParams closeParams = KeyboardGeometry.weightedHeightParams(s, 40, 1);
        LinearLayout.LayoutParams deleteParams = KeyboardGeometry.weightedHeightParams(s, 40, 1);
        deleteParams.setMarginStart(s.pixels(6));
        footer.addView(close, closeParams);
        footer.addView(delete, deleteParams);
        LinearLayout.LayoutParams footerParams = KeyboardGeometry.matchWidthWrapParams();
        footerParams.topMargin = s.pixels(6);
        s.expandedCandidates.addView(footer, footerParams);
    }
}
