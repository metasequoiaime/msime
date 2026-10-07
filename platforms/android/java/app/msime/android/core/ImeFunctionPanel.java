package app.msime.android;

import android.graphics.Color;
import android.os.Build;
import android.util.TypedValue;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.util.ArrayList;
import java.util.List;

/**
 * 功能面板（新设计的菜单，plan P25）：三页 4×2 的条目加页点，条目见 {@link FunctionPanelModel}。本地输入与「AI 回复与润色」各有一个子页。条目不可用时的外观由服务的 applyToolCardState 决定。
 *
 * <p>旧面板「开启态卡片渲染成深色块」的根因：开启的 TILE 卡片底色是 accentSoft（自定义皮肤里是 accent 加 0x24 透明度），经 `KeyboardSkinKeyDrawable` 绘制时选中卡片按 action 处理，`paint.setAlpha(opacity)` 把颜色自带的 0x24 透明度覆盖成 255，底色变成实心 accent，而文字 accentText 也是 accent，于是成了一块看不见字的深色块。新面板的条目不画底色（FunctionPanelView 挡掉样式通道给的键帽），开启态只用 accent 字形、加粗标签和 ✓ 角标表示。
 */
final class ImeFunctionPanel {
    private final MSIMEInputService s;

    ImeFunctionPanel(MSIMEInputService s) {
        this.s = s;
    }

    /** 品牌键：面板关着时打开，开着时回到键盘。 */
    void toggleFunctionPanel() {
        if (s.moreToolsScroll != null && s.moreToolsScroll.getVisibility() == View.VISIBLE) {
            s.closeMoreTools();
            s.render();
            return;
        }
        s.imePanels.showFeedbackMenu();
        s.render();
    }

    Button moreToolsCard(String title, MoreToolsLayout.Section section, boolean active,
                                 boolean enabled, boolean playBeforeAction, Runnable action) {
        Button card = toolButton();
        String state = enabled ? MoreToolsLayout.state(section, active) : "不可用";
        boolean navigates = section == MoreToolsLayout.Section.LOCAL_INPUT_BACK;
        String label = MoreToolsLayout.icon(title) + "  " + title;
        ViewPolicy.setTextSizeLabel(card, navigates ? label + "  ›" : label, 14);
        KeyboardGeometry.setKeyTextSize(card, 14);
        if (navigates) ViewPolicy.setStartCenteredVertically(card);
        else ViewPolicy.setCentered(card);
        KeyboardGeometry.setPaddingDp(card, s, 12, 5, 12, 5);
        card.setContentDescription(title);
        card.setSelected(active);
        card.setEnabled(enabled);
        s.applyToolCardState(card, enabled);
        if (Build.VERSION.SDK_INT >= 30) card.setStateDescription(state);
        s.imeStyler.styleButton(card, KeyboardKeyRole.ACCENT, s.skin);
        bindToolAction(card, action, playBeforeAction);
        return card;
    }

    void appendMoreToolsSection(MoreToolsLayout.Section section, Button... cards) {
        if (!section.title().isEmpty()) {
            TextView label = ViewPolicy.centeredText(s, section.title(), 11);
            s.moreToolsPanel.addView(label, KeyboardGeometry.matchWidthHeightPx(s.pixels(20)));
        }
        int columns = section.columns();
        for (int start = 0; start < cards.length; start += columns) {
            LinearLayout row = KeyboardGeometry.row(s);
            row.setWeightSum(columns);
            for (int column = 0; column < columns; column++) {
                int index = start + column;
                View child = index < cards.length ? cards[index] : new View(s);
                LinearLayout.LayoutParams params = KeyboardGeometry.weightedHeightPxParams(
                    s.pixels(section.height()), 1);
                if (column > 0) params.setMarginStart(s.pixels(MoreToolsLayout.CARD_SPACING_DP));
                row.addView(child, params);
            }
        LinearLayout.LayoutParams rowParams = KeyboardGeometry.matchWidthHeightPx(
            s.pixels(section.height()));
            rowParams.bottomMargin = s.pixels(MoreToolsLayout.ROW_SPACING_DP);
            s.moreToolsPanel.addView(row, rowParams);
        }
    }

    void renderMoreTools() {
        if (s.moreToolsPanel == null) return;
        s.moreToolsPanel.removeAllViews();
        // 子页没有 FunctionPanelView，由容器自己承担「更多工具」的描述，免得同一面板出现两个同名节点。
        s.moreToolsPanel.setContentDescription(
            s.localInputToolsOpen || s.aiAssistChooserOpen ? "更多工具" : null);
        if (s.localInputToolsOpen) {
            renderLocalInput();
            s.imeStyler.applySkin();
            return;
        }
        if (s.aiAssistChooserOpen) {
            renderAiAssistChooser();
            s.imeStyler.applySkin();
            return;
        }
        if (s.functionPanel == null) s.functionPanel = new FunctionPanelView(s);
        FunctionPanelView panel = s.functionPanel;
        if (panel.getParent() instanceof LinearLayout parent) parent.removeView(panel);
        int itemCapacity = FunctionPanelModel.items().size();
        List<FunctionPanelView.Entry> entries = new ArrayList<>(itemCapacity);
        List<FunctionPanelView.State> states = new ArrayList<>(itemCapacity);
        List<Boolean> enabled = new ArrayList<>(itemCapacity);
        for (FunctionPanelModel.Item item : FunctionPanelModel.items()) {
            addEntry(item, entries, states, enabled);
        }
        panel.setEntries(entries);
        for (int index = 0; index < entries.size(); index++) {
            Button tile = panel.entryView(index);
            boolean on = enabled.get(index);
            tile.setEnabled(on);
            s.applyToolCardState(tile, on);
            panel.setState(index, on ? states.get(index) : FunctionPanelView.State.UNAVAILABLE);
            if (!on) tile.setSelected(false);
        }
        KeyboardSkin skin = s.skin;
        panel.setColors(Color.parseColor(skin.keyForeground()), Color.parseColor(skin.accent()),
            Color.parseColor(skin.background()), Color.parseColor(skin.hairline()));
        s.moreToolsPanel.addView(panel, KeyboardGeometry.matchParentParams());
        s.imeStyler.applySkin();
    }

    private void addEntry(FunctionPanelModel.Item item, List<FunctionPanelView.Entry> entries,
                          List<FunctionPanelView.State> states, List<Boolean> enabled) {
        boolean ready = s.session != 0 && s.preferencesSnapshot != null;
        switch (item.id()) {
            case FULL_WIDTH -> add(entries, states, enabled, glyph(item, "全", s::toggleFullWidthInput),
                toggle(s.fullWidthInput), s.session != 0);
            case CHINESE_PUNCTUATION -> add(entries, states, enabled,
                glyph(item, "，", s::toggleChinesePunctuation), toggle(s.chinesePunctuation),
                s.session != 0);
            case FUZZY_PINYIN -> add(entries, states, enabled, glyph(item, "≈", s::toggleFuzzyPinyin),
                toggle(s.fuzzyPinyinEnabled), ready && !s.panelPreferenceSaving);
            case TRADITIONAL -> add(entries, states, enabled, glyph(item, "繁", s::toggleChineseOutput),
                toggle(s.traditionalChineseOutput), s.traditionalOutputToolAvailable());
            case HANDWRITING -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.HANDWRITING, () -> {
                    s.closeMoreTools();
                    s.toggleHandwritingScheme();
                }, null),
                FunctionPanelView.State.NONE, ready);
            case DICTIONARY -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.LEXICON, () -> host("LEXICON"), null),
                FunctionPanelView.State.NONE, true);
            case KEYBOARD_HEIGHT -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.KEYBOARD_HEIGHT, s::showInlineHeight, null),
                FunctionPanelView.State.NONE, ready);
            case SETTINGS -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.SETTINGS, () -> host(null), null),
                FunctionPanelView.State.NONE, true);
            case KEY_SOUND -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.KEY_SOUND, s::toggleSoundFromMoreTools, null),
                toggle(s.soundEnabled), true);
            case VIBRATION -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.VIBRATION, s::toggleHapticsFromMoreTools, null),
                toggle(s.hapticsEnabled), true);
            case ONE_HAND -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.ONE_HAND, () -> s.toggleOneHanded(false),
                    () -> s.toggleOneHanded(true)),
                toggle(!"off".equals(s.oneHandedMode)),
                // 分离式键盘画着的时候单手模式不生效，磁贴显示为不可用，免得点了没有反应。
                ready && !s.panelPreferenceSaving && !s.splitKeyboardDrawn());
            case PRIVACY -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.INCOGNITO, s::toggleIncognito, null),
                toggle(s.incognitoEnabled), ready && !s.panelPreferenceSaving);
            case FEEDBACK -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.FEEDBACK, () -> host("FEEDBACK"), null),
                FunctionPanelView.State.NONE, true);
            case ABOUT -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.ABOUT, () -> host("ABOUT"), null),
                FunctionPanelView.State.NONE, true);
            case AI_ASSIST -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.AI_ASSIST, () -> {
                    s.aiAssistChooserOpen = true;
                    renderMoreTools();
                }, null),
                FunctionPanelView.State.NONE, true);
            case LOCAL_INPUT -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.LOCAL_INPUT, () -> {
                    s.localInputToolsOpen = true;
                    renderMoreTools();
                }, null),
                FunctionPanelView.State.NONE, s.supportsLocalTools());
            case VOICE_RESULT -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.VOICE_RESULT, () -> {
                    s.closeMoreTools();
                    s.showVoiceResult();
                }, null),
                FunctionPanelView.State.NONE, true);
            case VIBRATION_STRENGTH -> add(entries, states, enabled,
                FunctionPanelView.Entry.icon(item.label() + " " + s.hapticStrengthTitle(),
                    item.description(), KeyboardIconPaths.Icon.VIBRATION_STRENGTH,
                    played(s::cycleHapticStrength), null),
                FunctionPanelView.State.NONE, s.hapticsEnabled);
            case EMOJI -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.KEY_EMOJI, () -> {
                    s.closeMoreTools();
                    s.imePanels.showEmojiPicker();
                    s.render();
                }, null),
                FunctionPanelView.State.NONE, s.session != 0 && !s.emojiResources.isEmpty());
            case CLIPBOARD -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.CLIPBOARD_HISTORY, () -> {
                    s.closeMoreTools();
                    s.imePanels.showClipboardHistory();
                    s.render();
                }, null),
                FunctionPanelView.State.NONE, CloudClipboardPanelPolicy.panelAvailable(
                    s.clipboardHistoryEnabled, s.imePanels.cloudClipboardAllowed()));
        }
    }

    private void host(String page) {
        s.closeMoreTools();
        s.openHostPage(page);
    }

    private static FunctionPanelView.State toggle(boolean on) {
        return on ? FunctionPanelView.State.ON : FunctionPanelView.State.OFF;
    }

    private static void add(List<FunctionPanelView.Entry> entries, List<FunctionPanelView.State> states,
                            List<Boolean> enabled, FunctionPanelView.Entry entry,
                            FunctionPanelView.State state, boolean available) {
        entries.add(entry);
        states.add(state);
        enabled.add(available);
    }

    private Runnable played(Runnable action) {
        return () -> {
            if (s.moreButton != null) s.imeKeyFeedback.playFeedback(s.moreButton);
            action.run();
        };
    }

    private FunctionPanelView.Entry glyph(FunctionPanelModel.Item item, String glyph, Runnable action) {
        return FunctionPanelView.Entry.glyph(item.label(), item.description(), glyph, played(action), null);
    }

    private FunctionPanelView.Entry icon(FunctionPanelModel.Item item, KeyboardIconPaths.Icon icon,
                                         Runnable action, Runnable longPress) {
        return FunctionPanelView.Entry.icon(item.label(), item.description(), icon, played(action),
            longPress);
    }

    /** 本地输入子页：「返回工具」加各个本地模式。 */
    private void renderLocalInput() {
        appendMoreToolsSection(MoreToolsLayout.Section.LOCAL_INPUT_BACK,
            moreToolsCard("返回工具", MoreToolsLayout.Section.LOCAL_INPUT_BACK,
                false, true, true, () -> {
                    s.localInputToolsOpen = false;
                    renderMoreTools();
                }));
        List<LocalInputMode> modes = s.localInputModes();
        Button[] localCards = new Button[modes.size()];
        for (int index = 0; index < modes.size(); index++) {
            LocalInputMode mode = modes.get(index);
            localCards[index] = moreToolsCard(mode.title(), MoreToolsLayout.Section.LOCAL_INPUT,
                false, s.supportsLocalTools() && s.localModeEnabled(mode), false, () -> {
                    s.closeMoreTools();
                    s.openLocalInputMode(mode);
                });
        }
        appendMoreToolsSection(MoreToolsLayout.Section.LOCAL_INPUT, localCards);
    }

    /** 「AI 回复与润色」子页：回复 | 润色两个分段，各自打开现有的回复键盘与 AI 润色面板。 */
    private void renderAiAssistChooser() {
        appendMoreToolsSection(MoreToolsLayout.Section.LOCAL_INPUT_BACK,
            moreToolsCard("返回工具", MoreToolsLayout.Section.LOCAL_INPUT_BACK,
                false, true, true, () -> {
                    s.aiAssistChooserOpen = false;
                    renderMoreTools();
                }));
        LinearLayout segments = KeyboardGeometry.row(s);
        segments.setContentDescription("AI 回复与润色");
        Button reply = segment("回复", "生成高情商回复", true, () -> {
            s.closeMoreTools();
            s.imePanels.showReplyKeyboard();
        });
        Button polish = segment("润色", "打开 AI 润色", s.aiPolishConfiguration != null
            && s.aiPolishReady(), () -> {
                s.closeMoreTools();
                s.imePanels.showAiPolish();
                s.render();
            });
        LinearLayout.LayoutParams first = KeyboardGeometry.weightedHeightPxParams(
            s.pixels(MoreToolsLayout.CARD_HEIGHT_DP), 1);
        LinearLayout.LayoutParams second = KeyboardGeometry.weightedHeightPxParams(
            s.pixels(MoreToolsLayout.CARD_HEIGHT_DP), 1);
        second.setMarginStart(s.pixels(MoreToolsLayout.CARD_SPACING_DP));
        segments.addView(reply, first);
        segments.addView(polish, second);
        s.moreToolsPanel.addView(segments, KeyboardGeometry.matchWidthWrapParams());
        TextView hint = ViewPolicy.textLabel(s,
            "回复：粘贴对方的话，生成几种语气的回复。润色：先选中要改的文字。", 12);
        KeyboardGeometry.setKeyTextSize(hint, 12);
        KeyboardGeometry.setPaddingDp(hint, s, 4, 10, 4, 0);
        s.moreToolsPanel.addView(hint);
    }

    private Button segment(String label, String description, boolean enabled, Runnable action) {
        KeyboardPressButton button = toolButton();
        button.setText(label);
        KeyboardGeometry.setKeyTextSize(button, 15);
        button.setContentDescription(description);
        button.setKeyboardRole(KeyboardKeyRole.ACCENT);
        button.setEnabled(enabled);
        s.applyToolCardState(button, enabled);
        if (Build.VERSION.SDK_INT >= 30) button.setStateDescription(enabled ? null : "不可用");
        bindToolAction(button, action, true);
        return button;
    }

    private KeyboardPressButton toolButton() {
        return ViewPolicy.newPressButton(s);
    }

    private void bindToolAction(Button button, Runnable action, boolean playFeedback) {
        button.setOnClickListener(ignored -> {
            if (playFeedback) s.imeKeyFeedback.playFeedback(button);
            action.run();
        });
    }
}
