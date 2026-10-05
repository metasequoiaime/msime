package app.msime.android;

import android.os.Build;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

/** 更多工具面板（功能面板）的卡片与分区；卡片禁用时的外观由服务的 applyToolCardState 决定。从 MSIMEInputService 原样搬出。 */
final class ImeFunctionPanel {
    private final MSIMEInputService s;

    ImeFunctionPanel(MSIMEInputService s) {
        this.s = s;
    }

    Button moreToolsCard(String title, MoreToolsLayout.Section section, boolean active,
                                 boolean enabled, boolean playBeforeAction, Runnable action) {
        return moreToolsCard(title, section, active, enabled, playBeforeAction, null, action);
    }

    Button moreToolsCard(String title, MoreToolsLayout.Section section, boolean active,
                                 boolean enabled, boolean playBeforeAction, String caption,
                                 Runnable action) {
        // Apple renders every tool card with the same press-feedback surface as a key. Keep the
        // Android card's existing state, accessibility and navigation behavior unchanged.
        Button card = new KeyboardPressButton(s);
        card.setAllCaps(false);
        String state = enabled ? MoreToolsLayout.state(section, active) : "不可用";
        String label = MoreToolsLayout.icon(title) + "  " + title;
        boolean tile = section.tiles();
        boolean navigates = section == MoreToolsLayout.Section.LOCAL_INPUT_BACK;
        // The design's function panel is a grid of icon-over-title tiles; an on setting is told by the tile's tint and its state description, and a caption (振动强度's level) follows the title.
        if (tile) card.setText(MoreToolsLayout.icon(title) + "\n" + title
            + (caption == null ? "" : " " + caption));
        else if (navigates) card.setText(label + "  ›");
        else card.setText(label);
        card.setTextSize(TypedValue.COMPLEX_UNIT_SP, tile ? 12 : 14);
        card.setGravity(navigates
            ? Gravity.CENTER_VERTICAL | Gravity.START : Gravity.CENTER);
        card.setPadding(s.pixels(tile ? 4 : 12), s.pixels(tile ? 4 : 5), s.pixels(tile ? 4 : 12),
            s.pixels(tile ? 4 : 5));
        if (tile) {
            card.setMaxLines(2);
            card.setLineSpacing(0, .95f);
        }
        card.setContentDescription(title);
        card.setSelected(active);
        card.setEnabled(enabled);
        s.applyToolCardState(card, enabled);
        if (Build.VERSION.SDK_INT >= 30) card.setStateDescription(state);
        if (tile && card instanceof KeyboardPressButton press)
            press.setKeyboardRole(KeyboardKeyRole.TILE);
        s.imeStyler.styleButton(card, tile ? KeyboardKeyRole.TILE : KeyboardKeyRole.ACCENT, s.skin);
        card.setOnClickListener(ignored -> {
            if (playBeforeAction) s.imeKeyFeedback.playFeedback(card);
            action.run();
        });
        return card;
    }

    void appendMoreToolsSection(MoreToolsLayout.Section section, Button... cards) {
        if (!section.title().isEmpty()) {
            TextView label = new TextView(s);
            label.setText(section.title());
            label.setTextSize(TypedValue.COMPLEX_UNIT_SP, 11);
            label.setGravity(Gravity.CENTER_VERTICAL);
            s.moreToolsPanel.addView(label, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(20)));
        }
        int columns = section.columns();
        for (int start = 0; start < cards.length; start += columns) {
            LinearLayout row = new LinearLayout(s);
            row.setWeightSum(columns);
            for (int column = 0; column < columns; column++) {
                int index = start + column;
                View child = index < cards.length ? cards[index] : new View(s);
                LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                    0, s.pixels(section.height()), 1);
                if (column > 0) params.setMarginStart(s.pixels(MoreToolsLayout.CARD_SPACING_DP));
                row.addView(child, params);
            }
            LinearLayout.LayoutParams rowParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(section.height()));
            rowParams.bottomMargin = s.pixels(MoreToolsLayout.ROW_SPACING_DP);
            s.moreToolsPanel.addView(row, rowParams);
        }
    }

    void renderMoreTools() {
        if (s.moreToolsPanel == null) return;
        s.moreToolsPanel.removeAllViews();
        LinearLayout header = new LinearLayout(s);
        header.setGravity(Gravity.CENTER_VERTICAL);
        Button close = s.button(header, "返回", s::closeMoreTools);
        close.setContentDescription("返回键盘");
        close.setLayoutParams(new LinearLayout.LayoutParams(
            s.pixels(84), s.pixels(MoreToolsLayout.HEADER_HEIGHT_DP)));
        TextView title = new TextView(s);
        title.setText("工具");
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        title.setGravity(Gravity.CENTER);
        header.addView(title, new LinearLayout.LayoutParams(
            0, s.pixels(MoreToolsLayout.HEADER_HEIGHT_DP), 1));
        View balance = new View(s);
        header.addView(balance, new LinearLayout.LayoutParams(
            s.pixels(84), s.pixels(MoreToolsLayout.HEADER_HEIGHT_DP)));
        s.moreToolsPanel.addView(header);

        if (s.localInputToolsOpen) {
            appendMoreToolsSection(MoreToolsLayout.Section.LOCAL_INPUT_BACK,
                moreToolsCard("返回工具", MoreToolsLayout.Section.LOCAL_INPUT_BACK,
                    false, true, true, () -> {
                        s.localInputToolsOpen = false;
                        renderMoreTools();
                    }));
            java.util.List<LocalInputMode> modes = s.localInputModes();
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
            s.imeStyler.applySkin();
            return;
        }

        appendMoreToolsSection(MoreToolsLayout.Section.TOOLS,
            moreToolsCard("表情", MoreToolsLayout.Section.TOOLS, false,
                s.session != 0 && !s.emojiResources.isEmpty(), true, () -> {
                    s.closeMoreTools();
                    s.imePanels.showEmojiPicker();
                }),
            moreToolsCard("剪贴板历史", MoreToolsLayout.Section.TOOLS, false,
                CloudClipboardPanelPolicy.panelAvailable(s.clipboardHistoryEnabled,
                    s.imePanels.cloudClipboardAllowed()), true, () -> {
                    s.closeMoreTools();
                    s.imePanels.showClipboardHistory();
                }),
            moreToolsCard("AI 润色", MoreToolsLayout.Section.TOOLS, false,
                s.aiPolishConfiguration != null && s.aiPolishReady(), true, () -> {
                    s.closeMoreTools();
                    s.imePanels.showAiPolish();
                }),
            moreToolsCard("本地输入", MoreToolsLayout.Section.TOOLS, false,
                s.supportsLocalTools(), true, () -> {
                    s.localInputToolsOpen = true;
                    renderMoreTools();
                }),
            moreToolsCard("语音结果", MoreToolsLayout.Section.TOOLS, false,
                true, true, () -> {
                    s.closeMoreTools();
                    s.showVoiceResult();
                }),
            // 键盘里改得了的只有这个面板上这些。皮肤、词库、账号、统计都在应用里，而用户正打着字，
            // 没有别的路走过去。
            moreToolsCard("应用设置", MoreToolsLayout.Section.TOOLS, false,
                true, true, () -> {
                    s.closeMoreTools();
                    s.openClientApp();
                }));
        appendMoreToolsSection(MoreToolsLayout.Section.SETTINGS,
            moreToolsCard("繁体输出", MoreToolsLayout.Section.SETTINGS, s.traditionalChineseOutput,
                s.traditionalOutputToolAvailable(), true, null, s::toggleChineseOutput),
            moreToolsCard("全角输入", MoreToolsLayout.Section.SETTINGS, s.fullWidthInput,
                true, false, s::toggleFullWidthInput),
            moreToolsCard("中文标点", MoreToolsLayout.Section.SETTINGS, s.chinesePunctuation,
                true, false, s::toggleChinesePunctuation),
            moreToolsCard("按键音", MoreToolsLayout.Section.SETTINGS, s.soundEnabled,
                true, false, s::toggleSoundFromMoreTools),
            moreToolsCard("按键振动", MoreToolsLayout.Section.SETTINGS, s.hapticsEnabled,
                true, false, s::toggleHapticsFromMoreTools),
            moreToolsCard("振动强度", MoreToolsLayout.Section.SETTINGS, s.hapticsEnabled,
                s.hapticsEnabled, false, s.hapticStrengthTitle(), s::cycleHapticStrength));
        s.imeStyler.applySkin();
    }
}
