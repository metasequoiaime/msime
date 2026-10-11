package app.msime.android;

import app.msime.android.LayoutPolicy;

import android.graphics.Color;
import android.os.Build;
import android.text.TextUtils;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import app.msime.android.core.InputViewValuePolicy;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 全拼九键的展开候选面板：点候选条右端的展开键后，键区换成三栏。左栏是与收起时侧栏相同的拼音列（可切成五个笔画键），中间是可纵向滚动的完整候选，右栏是 返回、⌫、重输、拼音/笔画、全部/单字。
 *
 * <p>选拼音、⌫ 和筛选都会让候选换一代，这个面板不因此收起，而是重新取完整候选再画；组字结束、点「返回」或选了候选才收起。筛选状态归引擎（`View.nine_key_single_character`、`View.nine_key_strokes`），面板只记左栏当前是拼音还是笔画。其他布局的展开面板仍是 {@link ImeCandidates#renderExpandedCandidates} 原来那块整面网格。
 */
final class ImeNineKeyPanel {
    private final MSIMEInputService s;
    private LinearLayout root;
    private FrameLayout left;
    private ScrollView spellingScroll;
    private LinearLayout spellingColumn;
    private LinearLayout strokeColumn;
    private TextView strokeReading;
    private ScrollView candidateScroll;
    private FrameLayout candidateHolder;
    private ImeCandidateGrid candidateGrid;
    private Button modeButton;
    private Button singleButton;
    private NineKeyPanelPolicy.Mode mode = NineKeyPanelPolicy.Mode.SPELLING;
    private long renderedGeneration = -1;
    private boolean filterResetPending;
    private long filterResetSession;
    private long filterResetGeneration = -1;

    ImeNineKeyPanel(MSIMEInputService s) {
        this.s = s;
    }

    /** 在创建键盘视图时建一次，和其他面板一样作为键盘表面的覆盖层，顶边由 alignOverlaysBelowTopRow 对齐到顶部一行下面。 */
    View build() {
        root = LayoutPolicy.row(s);
        root.setContentDescription("九键候选面板");
        ViewPolicy.setSymmetricPadding(root, s.pixels(3), s.pixels(4));

        left = new FrameLayout(s);
        spellingColumn = LayoutPolicy.column(s);
        spellingScroll = new ScrollView(s);
        spellingScroll.setVerticalScrollBarEnabled(false);
        spellingScroll.setContentDescription("九键拼音选择");
        spellingScroll.addView(spellingColumn, LayoutPolicy.scrollMatchWidthWrapParams());
        left.addView(spellingScroll, LayoutPolicy.frameMatchParentParams());
        strokeColumn = LayoutPolicy.column(s);
        strokeColumn.setContentDescription("笔画筛选");
        strokeReading = ViewPolicy.centeredText(s, "", 15);
        KeyboardTypographyPolicy.setKeyTextSize(strokeReading, 15);
        ViewPolicy.setSingleLine(strokeReading);
        strokeReading.setEllipsize(TextUtils.TruncateAt.START);
        ViewPolicy.setNonInteractive(strokeReading);
        strokeColumn.addView(strokeReading, LayoutPolicy.weightedWidthParams(1));
        for (StrokeKeyboardLayout.Key key : NineKeyPanelPolicy.strokes()) {
            Button stroke = pressButton(key.glyph(), 18, KeyboardKeyRole.PLAIN,
                "笔画筛选 " + key.name(), () -> appendStroke(key.input()));
            strokeColumn.addView(stroke, LayoutPolicy.weightedWidthParams(1));
        }
        ViewPolicy.hide(strokeColumn);
        left.addView(strokeColumn, LayoutPolicy.frameMatchParentParams());
        root.addView(left, margined(0.7f));

        candidateHolder = new FrameLayout(s);
        candidateScroll = new ScrollView(s);
        candidateScroll.setFillViewport(true);
        candidateScroll.addView(candidateHolder, LayoutPolicy.scrollMatchParentParams());
        candidateGrid = new ImeCandidateGrid(s, candidateScroll, false);
        root.addView(candidateScroll, margined(3));

        LinearLayout actions = LayoutPolicy.column(s);
        addAction(actions, pressButton("返回", 15, KeyboardKeyRole.ACCENT, "收起候选面板", () -> {
            s.closeCandidatePanel();
            s.render();
        }));
        addAction(actions, pressButton("⌫", 16, KeyboardKeyRole.ACCENT, "候选面板 删除", this::backspace));
        addAction(actions, pressButton("重输", 15, KeyboardKeyRole.ACCENT, "候选面板 重输",
            () -> s.command(3)));
        modeButton = pressButton("", 15, KeyboardKeyRole.ACCENT, "", this::toggleMode);
        addAction(actions, modeButton);
        singleButton = pressButton("", 15, KeyboardKeyRole.ACCENT, "", this::toggleSingleCharacter);
        addAction(actions, singleButton);
        root.addView(actions, margined(0.8f));
        ViewPolicy.hide(root);
        return root;
    }

    View root() {
        return root;
    }

    private LinearLayout.LayoutParams margined(float weight) {
        LinearLayout.LayoutParams params = LayoutPolicy.weightedMatchParentParams(weight);
        params.setMargins(s.pixels(2), 0, s.pixels(2), 0);
        return params;
    }

    private void addAction(LinearLayout actions, Button button) {
        LinearLayout.LayoutParams params = LayoutPolicy.weightedWidthParams(1);
        params.setMargins(0, s.pixels(2), 0, s.pixels(2));
        actions.addView(button, params);
    }

    private Button pressButton(String label, float sizeSp, KeyboardKeyRole role, String description,
                               Runnable action) {
        KeyboardPressButton button = ViewPolicy.newPressButton(s);
        KeyboardTypographyPolicy.normalizeKeyCap(button);
        button.setKeyboardRole(role);
        ViewPolicy.setTextSizeLabel(button, label, sizeSp);
        KeyboardTypographyPolicy.setKeyTextSize(button, sizeSp);
        ViewPolicy.setSingleLine(button);
        button.setContentDescription(description);
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            action.run();
        });
        return button;
    }

    /** 当前是否该画三栏：九键或 14 键字母键面上的全拼正在组字。 */
    boolean eligible() {
        JSONObject view = s.view;
        if (s.session == 0 || view == null) return false;
        return NineKeyPanelPolicy.threeColumn(
            s.displayedTouchLayout(view),
            s.keyboardLayer == KeyboardLayout.Layer.LETTERS,
            !"none".equals(InputViewValuePolicy.textOr(view, "local_mode", "none")),
            !InputViewValuePolicy.editingText(view).isEmpty());
    }

    /**
     * 让面板的完整候选跟上当前一代：代次没变就沿用，变了（选拼音、⌫、筛选）就重新取一次。取不到或会话已换时返回 false，由调用方收起面板。
     */
    boolean refreshSnapshot() {
        JSONObject snapshot = s.candidatePanelSnapshot;
        if (snapshot != null
                && CandidateGlossPolicy.strictOr(snapshot.opt("session"), Long.MIN_VALUE) == s.session
                && MSIMEInputService.sameCandidateVersion(snapshot, s.view)) return true;
        try {
            JSONObject next = s.nativeValue(NativeClient.allCandidates(s.session));
            if (CandidateGlossPolicy.strictOr(next.opt("session"), Long.MIN_VALUE) != s.session
                    || !MSIMEInputService.sameCandidateVersion(next, s.view)) return false;
            s.candidatePanelSnapshot = next;
            return true;
        } catch (JSONException | LinkageError error) {
            return false;
        }
    }

    /** 画三栏；调用前 refreshSnapshot 已经让 candidatePanelSnapshot 跟上了当前一代。 */
    void render() {
        if (root == null) return;
        ViewPolicy.show(root);
        long generation = CandidateGlossPolicy.strictOr(s.view.opt("generation"), -1);
        boolean newGeneration = generation != renderedGeneration;
        renderedGeneration = generation;
        renderLeft(newGeneration);
        renderCandidates();
        renderActions();
    }

    private void renderLeft(boolean newGeneration) {
        boolean strokes = mode == NineKeyPanelPolicy.Mode.STROKE;
        ViewPolicy.setVisible(spellingScroll, !strokes);
        ViewPolicy.setVisible(strokeColumn, strokes);
        if (strokes) {
            String current = strokesFilter();
            String glyphs = NineKeyPanelPolicy.glyphs(current);
            strokeReading.setText(glyphs.isEmpty() ? "笔画" : glyphs);
            strokeReading.setContentDescription(NineKeyPanelPolicy.strokesDescription(current));
            ViewPolicy.setActiveAlpha(strokeReading, !glyphs.isEmpty(), .5f);
            return;
        }
        spellingColumn.removeAllViews();
        ImeLayoutRows.Spellings spellings = ImeLayoutRows.Spellings.of(s.view);
        for (int slot = 0; slot < spellings.values().size(); slot++) {
            String spelling = spellings.values().get(slot);
            int index = spellings.indices().get(slot);
            long generation = spellings.generation();
            Button key = s.imeLayoutRows.spellingColumnButton(spellingColumn,
                () -> s.imeLayoutRows.chooseNineKeySpelling(generation, index));
            key.setText(spelling);
            key.setContentDescription(NineKeyPanelPolicy.spellingDescription(spelling, false));
        }
        if (newGeneration) spellingScroll.scrollTo(0, 0);
    }

    private void renderCandidates() {
        candidateHolder.removeAllViews();
        JSONObject snapshot = s.candidatePanelSnapshot;
        JSONArray entries = snapshot == null ? null : snapshot.optJSONArray("candidates");
        int count = entries == null ? 0 : entries.length();
        if (count == 0) {
            // 筛选可能一个候选也不剩；面板仍开着，好让用户撤掉一笔或换回全部。
            TextView empty = ViewPolicy.centeredText(s, "没有符合筛选的候选", 14);
            KeyboardTypographyPolicy.setKeyTextSize(empty, 14);
            ViewPolicy.setNonInteractive(empty);
            candidateHolder.addView(empty, LayoutPolicy.frameMatchWidthWrapParams(Gravity.CENTER));
            return;
        }
        String reading = InputViewValuePolicy.textOr(s.view, "nine_key_reading", "");
        // 格子分批建，滚到接近底部再追加；换了一代或面板收起后重开时，网格把滚动位置拉回顶部。
        CandidateWrapLayout list = candidateGrid.build(snapshot, entries,
            "完整候选列表；" + reading + "；" + count + " 个候选");
        LayoutPolicy.setSymmetricPaddingDp(list, s, 4, 4);
        candidateHolder.addView(list, LayoutPolicy.frameMatchWidthWrapParams());
    }

    private void renderActions() {
        modeButton.setText(NineKeyPanelPolicy.modeTitle(mode));
        modeButton.setContentDescription(NineKeyPanelPolicy.modeDescription(mode));
        ViewPolicy.setSelected(modeButton, mode == NineKeyPanelPolicy.Mode.STROKE);
        boolean single = singleCharacter();
        singleButton.setText(NineKeyPanelPolicy.singleCharacterTitle(single));
        singleButton.setContentDescription(NineKeyPanelPolicy.singleCharacterDescription(single));
        ViewPolicy.setSelected(singleButton, single);
        if (Build.VERSION.SDK_INT >= 30) {
            modeButton.setStateDescription(NineKeyPanelPolicy.modeTitle(mode));
            singleButton.setStateDescription(NineKeyPanelPolicy.singleCharacterTitle(single));
        }
    }

    /** 皮肤：整块面板用键盘底，左栏是和侧栏一样的圆角底条，中间候选区用候选面的底色。按钮由全键盘的样式通道按角色上色。 */
    void applySkin() {
        if (root == null) return;
        s.imeStyler.applySkinBackground(root);
        ViewPolicy.setBackground(left, DrawablePolicy.rounded(
            Color.parseColor(s.skin.sidebarBackground()), s.pixels(s.skin.cornerRadius())));
        ViewPolicy.setBackground(candidateScroll, DrawablePolicy.rounded(
            s.candidateAppearance.surface(), s.pixels(s.skin.cornerRadius())));
        int foreground = Color.parseColor(s.skin.keyForeground());
        ViewPolicy.setTextColor(strokeReading, foreground);
        for (int index = 0; index < candidateHolder.getChildCount(); index++) {
            if (candidateHolder.getChildAt(index) instanceof TextView empty)
                ViewPolicy.setTextColor(empty, s.candidateAppearance.text());
        }
    }

    /**
     * 面板不再显示：收起三栏，左栏回到拼音。组字还在而筛选开着时把筛选清掉，收起后的候选条不带筛选。
     *
     * <p>清筛选要经 apply 重画，而这里可能正是从 render 里调到的（候选面板在重画时发现自己该收起），所以排到主线程下一轮再做，到时面板又开了或筛选已没了就什么也不做。
     */
    void dismiss() {
        mode = NineKeyPanelPolicy.Mode.SPELLING;
        renderedGeneration = -1;
        if (candidateGrid != null) candidateGrid.reset();
        // 键盘视图建好之前（onStartInput 经 stop 走到这里）面板还没有 root；isGone 对 null 为假，不先判空 hide 就会在每次弹出键盘前崩溃。
        if (root != null && !ViewPolicy.isGone(root)) ViewPolicy.hide(root);
        if (filterResetPending || !filtered()) return;
        // 同一代只清一次：清除的响应会再走到这里，引擎万一没清掉也不会每次重画都再发一遍。
        long generation = CandidateGlossPolicy.strictOr(s.view.opt("generation"), -1);
        if (s.session == filterResetSession && generation == filterResetGeneration) return;
        filterResetPending = true;
        s.main.post(() -> {
            filterResetPending = false;
            if (s.candidatePanelOpen || s.session == 0 || !filtered()) return;
            filterResetSession = s.session;
            filterResetGeneration = CandidateGlossPolicy.strictOr(s.view.opt("generation"), -1);
            s.setNineKeyFilter(false, "");
        });
    }

    private boolean filtered() {
        return singleCharacter() || !strokesFilter().isEmpty();
    }

    private boolean singleCharacter() {
        return s.view != null
            && InputViewValuePolicy.booleanValue(s.view, "nine_key_single_character", false);
    }

    private String strokesFilter() {
        return InputViewValuePolicy.textOr(s.view, "nine_key_strokes", "");
    }

    private void backspace() {
        String strokes = strokesFilter();
        if (NineKeyPanelPolicy.backspace(mode, strokes) == NineKeyPanelPolicy.Backspace.POP_STROKE) {
            s.setNineKeyFilter(singleCharacter(), NineKeyPanelPolicy.popStroke(strokes));
            return;
        }
        if (s.connection != null && !s.command(0)) s.deleteCodePointBeforeCursor();
    }

    private void appendStroke(char code) {
        String strokes = strokesFilter();
        String next = NineKeyPanelPolicy.appendStroke(strokes, code);
        if (!next.equals(strokes)) s.setNineKeyFilter(singleCharacter(), next);
    }

    private void toggleMode() {
        String cleared = NineKeyPanelPolicy.strokesAfterToggle(mode, strokesFilter());
        mode = NineKeyPanelPolicy.toggledMode(mode);
        if (cleared != null) s.setNineKeyFilter(singleCharacter(), cleared);
        else s.render();
    }

    private void toggleSingleCharacter() {
        s.setNineKeyFilter(!singleCharacter(), strokesFilter());
    }
}
