package app.msime.android;

import app.msime.android.LayoutPolicy;

import android.graphics.Color;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;
import java.util.ArrayList;
import java.util.List;

/**
 * 功能菜单：四列，手机竖屏和平板三行、手机横屏两行（{@link MSIMEInputService#pickerGrid}），常用操作在前、其他工具在后，过滤顶部已有的入口；条目见 {@link FunctionPanelModel}。本地输入与「AI 回复与润色」各有一个子页。
 *
 * <p>旧面板「开启态卡片渲染成深色块」的根因：开启的 TILE 卡片底色是 accentSoft（自定义皮肤里是 accent 加 0x24 透明度），经 `KeyboardSkinKeyDrawable` 绘制时选中卡片按 action 处理，`paint.setAlpha(opacity)` 把颜色自带的 0x24 透明度覆盖成 255，底色变成实心 accent，而文字 accentText 也是 accent，于是成了一块看不见字的深色块。面板条目不画底色（FunctionPanelView 挡掉样式通道给的键帽），开启态用强调色和勾标表示。
 */
final class ImeFunctionPanel {
    private final MSIMEInputService s;
    /** 菜单仍打开且入口没有变化时复用磁贴，避免每次键盘刷新都闪一下。 */
    private java.util.List<FunctionPanelModel.Id> renderedItemIds = java.util.List.of();

    private FunctionPanelView chooserPanel;
    private String chooserTitle = "";
    private List<String> chooserLabels = List.of();

    ImeFunctionPanel(MSIMEInputService s) {
        this.s = s;
    }

    /** 品牌键：面板关着时打开，开着时回到键盘。 */
    void toggleFunctionPanel() {
        if (ViewPolicy.isVisible(s.moreToolsScroll)) {
            s.closeMoreTools();
            s.render();
            return;
        }
        s.imePanels.showFeedbackMenu();
        s.render();
    }

    void renderMoreTools() {
        if (s.moreToolsPanel == null) return;
        if (s.imeWorldClock != null && s.imeWorldClock.active()) {
            s.imeWorldClock.render();
            return;
        }
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
        if (s.functionPanel == null) {
            s.functionPanel = new FunctionPanelView(s);
            renderedItemIds = List.of();
        }
        FunctionPanelView panel = s.functionPanel;
        panel.setPickerGrid(s.pickerGrid(FunctionPanelView.ITEM_HEIGHT_DP,
            FunctionPanelView.CHROME_DP + MSIMEInputService.MORE_TOOLS_BOTTOM_PADDING_DP));
        java.util.Set<FunctionPanelModel.Id> hiddenItems = java.util.EnumSet.noneOf(FunctionPanelModel.Id.class);
        if (!s.toolbarHidden) {
            if (s.toolbarEmoji) hiddenItems.add(FunctionPanelModel.Id.EMOJI);
            if (s.toolbarClipboard) hiddenItems.add(FunctionPanelModel.Id.CLIPBOARD);
            if (s.toolbarFloating) hiddenItems.add(FunctionPanelModel.Id.FLOATING);
            if (s.toolbarTextEdit) hiddenItems.add(FunctionPanelModel.Id.TEXT_EDIT);
        }
        if (!s.hasPendingVoiceResult()) hiddenItems.add(FunctionPanelModel.Id.VOICE_RESULT);
        List<FunctionPanelModel.Item> visibleItems = FunctionPanelModel.itemsExcept(hiddenItems);
        java.util.ArrayList<FunctionPanelModel.Id> itemIds = new java.util.ArrayList<>(visibleItems.size());
        for (FunctionPanelModel.Item item : visibleItems) itemIds.add(item.id());
        boolean entriesChanged = !itemIds.equals(renderedItemIds);
        List<FunctionPanelView.Entry> entries = new ArrayList<>(visibleItems.size());
        List<FunctionPanelView.State> states = new ArrayList<>(visibleItems.size());
        List<Boolean> enabled = new ArrayList<>(visibleItems.size());
        for (FunctionPanelModel.Item item : visibleItems) {
            addEntry(item, entries, states, enabled);
        }
        if (entriesChanged) {
            panel.setEntries(entries);
            renderedItemIds = List.copyOf(itemIds);
        } else {
            panel.updateEntries(entries);
        }
        // setEntries 只在入口集合变化时调用；这样开关状态更新不会摘掉正在按压的磁贴。
        for (int index = 0; index < entries.size(); index++) {
            Button tile = panel.entryView(index);
            boolean on = enabled.get(index);
            ViewPolicy.setEnabled(tile, on);
            s.applyToolCardState(tile, on);
            panel.setState(index, on ? states.get(index) : FunctionPanelView.State.UNAVAILABLE);
            if (!on) ViewPolicy.setSelected(tile, false);
        }
        KeyboardSkin skin = s.skin;
        panel.setColors(Color.parseColor(skin.keyForeground()), Color.parseColor(skin.accent()),
            Color.parseColor(skin.background()), Color.parseColor(skin.hairline()));
        if (panel.getParent() != s.moreToolsPanel) {
            if (panel.getParent() instanceof android.view.ViewGroup parent) parent.removeView(panel);
            s.moreToolsPanel.removeAllViews();
            s.moreToolsPanel.addView(panel, LayoutPolicy.matchParentParams());
        }
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
            case WORLD_CLOCK -> add(entries, states, enabled,
                glyph(item, "◷", s.imeWorldClock::open),
                FunctionPanelView.State.NONE, s.connection != null);
            case TOOLBAR_EDITOR -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.SETTINGS, s.imeToolbar::openEditor, null),
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
                // 分离式键盘画着或键盘浮动的时候单手模式不生效，磁贴显示为不可用，免得点了没有反应。
                ready && !s.panelPreferenceSaving && !s.splitKeyboardDrawn() && !s.floatingDrawn());
            case PRIVACY -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.INCOGNITO, s::toggleIncognito, null),
                toggle(s.incognitoEnabled), ready && !s.panelPreferenceSaving);
            case FEEDBACK -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.FEEDBACK, () -> host("FEEDBACK"), null),
                FunctionPanelView.State.NONE, true);
            case COPY_DEVICE_INFO -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.DEVICE_INFO, s::copyDeviceInfo, null),
                FunctionPanelView.State.NONE, true);
            case PROMOTION -> add(entries, states, enabled,
                glyph(item, "吆", () -> {
                    s.closeMoreTools();
                    s.command(2);
                    s.commitText(PromotionText.TEXT);
                    s.render();
                }), FunctionPanelView.State.NONE, s.connection != null);
            case ABOUT -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.ABOUT, () -> host("ABOUT"), null),
                FunctionPanelView.State.NONE, true);
            case WEBSITE -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.WEBSITE, s::openWebsite, null),
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
            case VIBRATION_STRENGTH -> add(entries, states, enabled,
                FunctionPanelView.Entry.icon(item.label() + " " + s.hapticStrengthTitle(),
                    item.description(), KeyboardIconPaths.Icon.VIBRATION_STRENGTH,
                    played(s::cycleHapticStrength), null),
                FunctionPanelView.State.NONE, s.hapticsEnabled);
            case VOICE_RESULT -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.VOICE_RESULT, () -> {
                    s.closeMoreTools();
                    s.showVoiceResult();
                }, null),
                FunctionPanelView.State.NONE, s.voiceInsertionReady());
            case EMOJI -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.KEY_EMOJI, () -> {
                    s.closeMoreTools();
                    s.imePanels.showEmojiPicker();
                    s.render();
                }, null),
                FunctionPanelView.State.NONE, s.session != 0 && !s.emojiResources.isEmpty());
            case FLOATING -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.FLOATING, s::toggleFloatingKeyboard, null),
                toggle(s.floatingKeyboard),
                // 外接键盘的候选条模式里浮动不生效。
                !s.panelPreferenceSaving && !s.hardwareKeyboardMode);
            case CLIPBOARD -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.CLIPBOARD_HISTORY, () -> {
                    s.closeMoreTools();
                    s.imePanels.showClipboardHistory();
                    s.render();
                }, null),
                FunctionPanelView.State.NONE, CloudClipboardPanelPolicy.panelAvailable(
                    s.clipboardHistoryEnabled, s.imePanels.cloudClipboardAllowed()));
            case TEXT_EDIT -> add(entries, states, enabled,
                icon(item, KeyboardIconPaths.Icon.TEXT_EDIT, () -> {
                    s.closeMoreTools();
                    s.imeTextEditPanel.show();
                    s.render();
                }, null),
                FunctionPanelView.State.NONE, s.connection != null);
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

    /** 本地输入沿用主菜单图标网格，三列三行，不再给每项铺一块重底色。 */
    private void renderLocalInput() {
        List<FunctionPanelView.Entry> entries = new ArrayList<>();
        List<Boolean> enabled = new ArrayList<>();
        for (LocalInputMode mode : s.localInputModes()) {
            String glyph = switch (mode) {
                case UNICODE -> "U+";
                case DATE_TIME -> "时";
                case SUPER_JIANPIN -> "简";
                case QUICK_PHRASE -> "词";
                case TEMPORARY_ENGLISH -> "英";
                case EMOJI -> "☺";
                case KAOMOJI -> "顔";
                case TEMPORARY_JAPANESE -> "あ";
                case CHAIZI -> "拆";
            };
            entries.add(FunctionPanelView.Entry.glyph(mode.title(), mode.title(), glyph, () -> {
                s.closeMoreTools();
                s.openLocalInputMode(mode);
            }, null));
            enabled.add(s.supportsLocalTools() && s.localModeEnabled(mode));
        }
        renderChooser("本地输入", entries, enabled, 3, 3);
    }

    /** AI 辅助只呈现回复与润色两个入口，图标和标签与主菜单一致。 */
    private void renderAiAssistChooser() {
        List<FunctionPanelView.Entry> entries = List.of(
            FunctionPanelView.Entry.icon("回复", "生成高情商回复", KeyboardIconPaths.Icon.TOOLBAR_PHRASE,
                played(() -> {
                    s.closeMoreTools();
                    s.imePanels.showReplyKeyboard();
                }), null),
            FunctionPanelView.Entry.icon("润色", "打开 AI 润色", KeyboardIconPaths.Icon.AI_ASSIST,
                played(() -> {
                    s.closeMoreTools();
                    s.imePanels.showAiPolish();
                    s.render();
                }), null));
        renderChooser("AI 辅助", entries, List.of(true,
            s.aiPolishConfiguration != null && s.aiPolishReady()), 2, 1);
    }

    /** 子页用同一紧凑标题栏；刷新只更新状态，保留条目与点按区域。 */
    private void renderChooser(String title, List<FunctionPanelView.Entry> entries,
                               List<Boolean> enabled, int columns, int rows) {
        List<String> labels = new ArrayList<>(entries.size());
        for (FunctionPanelView.Entry entry : entries) labels.add(entry.label);
        boolean changed = chooserPanel == null || !title.equals(chooserTitle)
            || !labels.equals(chooserLabels);
        if (changed) {
            chooserPanel = new FunctionPanelView(s);
            chooserPanel.setContentDescription(null);
            chooserPanel.grid().setGrid(columns, rows);
            chooserPanel.grid().setSpacing(52, 8, 8, 4);
            chooserPanel.setEntries(entries);
            chooserTitle = title;
            chooserLabels = List.copyOf(labels);
        } else {
            chooserPanel.updateEntries(entries);
        }
        for (int index = 0; index < entries.size(); index++) {
            Button tile = chooserPanel.entryView(index);
            ViewPolicy.setEnabled(tile, enabled.get(index));
            s.applyToolCardState(tile, enabled.get(index));
            chooserPanel.setState(index, enabled.get(index)
                ? FunctionPanelView.State.NONE : FunctionPanelView.State.UNAVAILABLE);
        }
        chooserPanel.setColors(Color.parseColor(s.skin.keyForeground()), Color.parseColor(s.skin.accent()),
            Color.parseColor(s.skin.background()), Color.parseColor(s.skin.hairline()));
        if (chooserPanel.getParent() == s.moreToolsPanel) return;
        if (chooserPanel.getParent() instanceof android.view.ViewGroup parent) parent.removeView(chooserPanel);
        s.moreToolsPanel.removeAllViews();
        LinearLayout header = LayoutPolicy.row(s);
        KeyboardIconKey back = new KeyboardIconKey(s, KeyboardIconKey.Kind.CURSOR_LEFT);
        back.setKeyboardRole(KeyboardKeyRole.PLAIN);
        back.setText("返回工具");
        back.setContentDescription("返回工具");
        ViewPolicy.bindClick(back, played(() -> {
            s.localInputToolsOpen = false;
            s.aiAssistChooserOpen = false;
            renderMoreTools();
        }));
        header.addView(back, LayoutPolicy.linearParamsPx(s.pixels(44), s.pixels(44)));
        TextView heading = ViewPolicy.centeredText(s, title, 15);
        KeyboardTypographyPolicy.setKeyTextSize(heading, 15);
        header.addView(heading, LayoutPolicy.weightedHeightPxParams(s.pixels(44), 1));
        header.addView(new View(s), LayoutPolicy.linearParamsPx(s.pixels(44), s.pixels(44)));
        s.moreToolsPanel.addView(header, LayoutPolicy.matchWidthHeightPx(s.pixels(44)));
        s.moreToolsPanel.addView(chooserPanel, LayoutPolicy.matchWidthHeightPx(
            s.pixels(rows * 52 + (rows - 1) * 8 + 14)));
        s.moreToolsScroll.scrollTo(0, 0);
    }
}
