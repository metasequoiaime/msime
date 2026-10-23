package app.msime.android;

import app.msime.android.LayoutPolicy;

import android.graphics.Color;
import android.os.SystemClock;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.inputmethod.InputConnection;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * 键盘顶部一行（设计 50 dp）：空闲时是工具栏（品牌、表情、常用语、剪贴板、皮肤、输入方式、收起），组词时是候选条（读音 + 候选 chip + 分隔线 + 展开键），调整键盘高度时是内联高度条。状态仍在服务里。
 */
final class ImeToolbar {
    /** 组词时读音那一行的最小高度（设计稿按 12 sp 的读音排的 14 dp）。读音字号跟设置「预编辑字号」（`candidate_preedit_font_size`）按比例缩放（ReadingRowPolicy.textSizeSp，默认仍是 12 sp），字号变大、系统字体放大或厂商字体更高时按读音实际高度加上 {@link #READING_GAP_DP} 加高，见 ReadingRowPolicy；空闲时的工具栏取读音行加候选行的同一高度，打字时键盘不变高。 */
    static final int READING_ROW_DP = 14;
    /** 读音和下面候选行之间固定留的间距（#6107），是读音行的下内边距，计入读音行高度，所以键盘总高随之多出这几 dp。 */
    static final int READING_GAP_DP = 3;
    /** 候选 chip 那一行的高度：候选字加一行 0.62 倍的释义和选中 chip 的上下留白。设计是 34 dp，实测 34、36 dp 时选中的 chip 和释义都会伸出候选行压到下面的键，所以取 42 dp。 */
    static final int CANDIDATE_LINE_DP = 42;
    /** 多出一行释义时每行加的高度。 */
    static final int EXTRA_GLOSS_ROW_DP = 14;

    private final MSIMEInputService s;
    private Button[] shortcutButtons;
    private java.util.List<ToolbarOrderPolicy.Item> appliedOrder;
    /** 「最近复制」那一行自己的剪贴板入口和收起键，和工具栏上的同一套配色。 */
    private Button recentClipHistoryButton;
    private Button recentClipCollapseButton;
    private String styleCacheKey;
    private int iconColor;
    private int activeIconColor;
    private int activeBackgroundColor;
    private int foregroundColor;
    private int hairlineColor;
    private int hintColor;
    private int returnBackgroundColor;
    private int returnForegroundColor;

    ImeToolbar(MSIMEInputService s) {
        this.s = s;
    }

    /** 空闲工具栏：品牌、表情、常用语、剪贴板、皮肤、输入方式、浮动键盘、文本编辑、收起；两端固定，中间工具等分剩余宽度，放不下时横向滚动；哪些显示由 render 按 `touch_toolbar` 与本地设置决定。数字键面上有算式结果时，品牌键右边多一个结果胶囊（{@link ImeCalculator}）。 */
    void installShortcutBar(Button dismissButton) {
        appliedOrder = null;
        s.shortcutBar.removeAllViews();
        s.dismissShortcutButton = dismissButton;
        shortcutButtons = new Button[] {s.emojiShortcutButton, s.phraseShortcutButton,
            s.clipboardShortcutButton, s.skinButton, s.schemeButton, s.floatingShortcutButton,
            s.textEditShortcutButton, s.undoShortcutButton, s.redoShortcutButton};
        for (Button button : shortcutButtons) bindEditor(button);
        s.imeTextEditPanel.bindShortcut(s.textEditShortcutButton);
        for (Button button : new Button[] {s.moreButton, dismissButton}) {
            bindEditor(button);
            if (button.getParent() instanceof LinearLayout parent) parent.removeView(button);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(s.pixels(44), s.pixels(44));
            params.setMarginStart(s.pixels(1));
            params.setMarginEnd(s.pixels(1));
            s.shortcutToolbar.addView(button, button == s.moreButton ? 0 : s.shortcutToolbar.getChildCount(), params);
            ViewPolicy.setMinimumWidth(button, s.pixels(40));
        }
        // 数字键面的计算结果（#5688）放在中间滚动区最前面；没有结果时隐藏，不占宽度。
        Button result = s.imeCalculator.chip();
        if (result.getParent() instanceof LinearLayout parent) parent.removeView(result);
        s.shortcutBar.addView(result, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, s.pixels(44)));
        for (Button button : shortcutButtons) {
            if (button.getParent() instanceof LinearLayout parent) parent.removeView(button);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(s.pixels(40), s.pixels(44), 1);
            params.setMarginStart(s.pixels(1));
            params.setMarginEnd(s.pixels(1));
            s.shortcutBar.addView(button, params);
            ViewPolicy.setMinimumWidth(button, s.pixels(40));
        }
        // 旧的回复、语音、简繁、AI 润色、⚙ 入口不在新工具栏上：AI 在功能面板第 2 页，语音由长按空格进入，设置在功能面板里。按钮对象保留，服务里其余代码照常更新它们的状态。
        for (Button retired : new Button[] {s.scriptShortcutButton, s.aiPolishShortcutButton,
                s.replyShortcutButton, s.voiceShortcutButton, s.layoutSettingsButton}) {
            if (retired == null) continue;
            if (retired.getParent() instanceof LinearLayout parent) parent.removeView(retired);
            ViewPolicy.hide(retired);
        }
    }

    /** 长按由框架消费，不再触发该按钮原来的短按动作。 */
    private void bindEditor(Button button) {
        button.setOnLongClickListener(view -> {
            s.imeKeyFeedback.playFeedback(view);
            openEditor();
            return true;
        });
    }

    void openEditor() {
        s.closeMoreTools();
        s.openHostPage("TOOLBAR_EDITOR");
    }

    /** 只移动已有视图，保留按钮原来的回调、触摸区域、无障碍身份与面板激活态。 */
    private void applyShortcutOrder() {
        if (shortcutButtons == null || s.shortcutBar == null) return;
        java.util.List<ToolbarOrderPolicy.Item> order = ToolbarOrderPolicy.ordered(
            s.localSettings.text(AndroidLocalSettings.TOOLBAR_ORDER));
        if (order.equals(appliedOrder)) return;
        for (Button button : shortcutButtons) s.shortcutBar.removeView(button);
        for (int index = 0; index < order.size(); index++) {
            Button button = switch (order.get(index)) {
                case EMOJI -> s.emojiShortcutButton;
                case PHRASE -> s.phraseShortcutButton;
                case CLIPBOARD -> s.clipboardShortcutButton;
                case SKIN -> s.skinButton;
                case SCHEME -> s.schemeButton;
                case FLOATING -> s.floatingShortcutButton;
                case TEXT_EDIT -> s.textEditShortcutButton;
                case UNDO -> s.undoShortcutButton;
                case REDO -> s.redoShortcutButton;
            };
            // 算式结果保留为中间滚动区的第一项，两端按钮不参与排序。
            s.shortcutBar.addView(button, index + 1);
        }
        appliedOrder = order;
    }

    /** 读音行（读音、提示、页码、漢、退出本地模式）与工具栏的滚动容器；读音行只在组词或有提示时显示。 */
    void buildCandidateHeader(LinearLayout candidateRegion) {
        LinearLayout candidateHeader = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(candidateHeader);
        LayoutPolicy.setPaddingDp(candidateHeader, s, 10, 0, 6, READING_GAP_DP);
        s.candidateHeader = candidateHeader;
        s.preedit = toolbarText(12);
        ViewPolicy.setMaxLinesEllipsized(s.preedit, 1);
        // 胶囊紧挨着候选栏和按键，轻点很容易误触，所以只在长按时打开本地模式菜单。
        s.preedit.setOnLongClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.preedit);
            s.imePanels.showLocalInputMenu();
            return true;
        });
        // 点击现有标题进入临时放大编辑，不在细小的标题里直接定位。
        s.preedit.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.preedit);
            s.imeCompositionEditor.open();
        });
        ViewPolicy.setClickable(s.preedit, false);
        LinearLayout preeditFrame = LayoutPolicy.row(s);
        ViewPolicy.setStartCenteredVertically(preeditFrame);
        preeditFrame.addView(s.preedit, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        candidateHeader.addView(preeditFrame, LayoutPolicy.weightedWrapParams(1));
        // 宿主提示通道：正常为空，只有准备中、失败或提示时才有文字。
        s.status = toolbarText(10);
        ViewPolicy.setMaxLinesEllipsized(s.status, 1);
        ViewPolicy.setEndCenteredVertically(s.status);
        LayoutPolicy.setPaddingDp(s.status, s, 6, 0, 2, 0);
        candidateHeader.addView(s.status, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        s.candidatePage = toolbarText(10);
        candidateHeader.addView(s.candidatePage, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        s.shortcutBar = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(s.shortcutBar);
        s.shortcutToolbar = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(s.shortcutToolbar);
        s.shortcutToolbar.setContentDescription("键盘快捷栏");
        LayoutPolicy.setPaddingDp(s.shortcutToolbar, s, 2, 0, 2, 0);
        s.shortcutScroll = new HorizontalScrollView(s);
        s.shortcutScroll.setHorizontalScrollBarEnabled(false);
        s.shortcutScroll.setContentDescription("可滑动键盘工具");
        s.shortcutScroll.setFillViewport(true);
        s.shortcutScroll.addView(s.shortcutBar, new HorizontalScrollView.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));
        s.shortcutToolbar.addView(s.shortcutScroll, new LinearLayout.LayoutParams(
            0, LinearLayout.LayoutParams.MATCH_PARENT, 1));
        s.scriptShortcutButton = s.button(s.shortcutBar, "简", s::toggleChineseOutput);
        s.scriptShortcutButton.setContentDescription("切换到繁体");
        s.emojiShortcutButton = s.shortcutButton(s.shortcutBar, "☺",
            KeyboardShortcutIconPolicy.Icon.EMOJI,
            panelToggle(() -> s.emojiPanel, s.imePanels::showEmojiPicker));
        s.emojiShortcutButton.setContentDescription("表情");
        s.keyId(s.emojiShortcutButton, "SoftEmoji");
        s.phraseShortcutButton = s.shortcutButton(s.shortcutBar, "常用语",
            KeyboardShortcutIconPolicy.Icon.PHRASE,
            panelToggle(() -> s.phraseScroll, s.imePanels::showCommonPhrases));
        s.phraseShortcutButton.setContentDescription("常用语");
        s.clipboardShortcutButton = s.shortcutButton(s.shortcutBar, "剪贴板",
            KeyboardShortcutIconPolicy.Icon.CLIPBOARD,
            panelToggle(() -> s.clipboardSurface, s.imePanels::showClipboardHistory));
        s.clipboardShortcutButton.setContentDescription("剪贴板");
        // 浮动键盘按钮默认不在工具栏上，用户在设置「键盘工具栏」里打开（#5621）。
        s.floatingShortcutButton = s.shortcutButton(s.shortcutBar, "浮动键盘",
            KeyboardShortcutIconPolicy.Icon.FLOATING, s::toggleFloatingKeyboard);
        s.floatingShortcutButton.setContentDescription("浮动键盘");
        // 文本编辑按钮同样默认不在工具栏上（#6351），点按开关文本编辑面板，与功能面板里的「文本编辑」是同一个面板。
        s.textEditShortcutButton = s.shortcutButton(s.shortcutBar, "文本编辑",
            KeyboardShortcutIconPolicy.Icon.TEXT_EDIT,
            panelToggle(() -> s.textEditPanel, s.imeTextEditPanel::show));
        s.textEditShortcutButton.setContentDescription("文本编辑");
        s.undoShortcutButton = s.shortcutButton(s.shortcutBar, "撤销",
            KeyboardShortcutIconPolicy.Icon.UNDO, () -> performHistory(false));
        s.undoShortcutButton.setContentDescription("撤销");
        s.redoShortcutButton = s.shortcutButton(s.shortcutBar, "重做",
            KeyboardShortcutIconPolicy.Icon.REDO, () -> performHistory(true));
        s.redoShortcutButton.setContentDescription("重做");
        s.voiceShortcutButton = s.shortcutButton(s.shortcutBar, "语音",
            KeyboardShortcutIconPolicy.Icon.VOICE, s::showVoiceResult);
        s.voiceShortcutButton.setContentDescription("打开语音结果");
        s.keyId(s.voiceShortcutButton, "SoftVoice");
        s.aiPolishShortcutButton = s.button(s.shortcutBar, "AI", s.imePanels::showAiPolish);
        s.aiPolishShortcutButton.setContentDescription("打开 AI 润色");
        s.replyShortcutButton = s.shortcutButton(s.shortcutBar, "回复",
            KeyboardShortcutIconPolicy.Icon.REPLY, s::toggleReplyKeyboard);
        s.replyShortcutButton.setContentDescription("生成高情商回复");
        // 漢 是韩语键盘 Hanja 键的触屏对应：列出组字音节的汉字，再点一次关上；注音组字时同一个键读作 選，经共享命令 16 打开转换列表。只在这两种情况下由 render 显示。
        KeyboardPressButton hanja = new KeyboardPressButton(s);
        hanja.setKeyboardRole(KeyboardKeyRole.GLYPH);
        s.hanjaButton = hanja;
        ViewPolicy.setAllCapsFalse(s.hanjaButton);
        s.hanjaButton.setText("漢");
        KeyboardTypographyPolicy.setKeyTextSize(s.hanjaButton, 12);
        s.hanjaButton.setContentDescription("转换为汉字");
        ViewPolicy.hide(s.hanjaButton);
        bindToolbarAction(s.hanjaButton,
            () -> s.command(KoreanInputPolicy.CONVERT_HANJA_COMMAND));
        LayoutPolicy.setHorizontalPaddingDp(s.hanjaButton, s, 8);
        ViewPolicy.clearMinimumHeight(s.hanjaButton);
        candidateHeader.addView(s.hanjaButton, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));
        s.exitLocalModeButton = new KeyboardBorderlessButton(s);
        ViewPolicy.setAllCapsFalse(s.exitLocalModeButton);
        s.exitLocalModeButton.setText("×");
        KeyboardTypographyPolicy.setKeyTextSize(s.exitLocalModeButton, 14);
        s.exitLocalModeButton.setContentDescription("退出本地模式");
        ViewPolicy.clearPadding(s.exitLocalModeButton);
        s.imeStyler.styleButton(s.exitLocalModeButton, true);
        bindToolbarAction(s.exitLocalModeButton, s::clearCurrentInput);
        ViewPolicy.clearMinimumHeight(s.exitLocalModeButton);
        candidateHeader.addView(s.exitLocalModeButton, new LinearLayout.LayoutParams(
            s.pixels(32), LinearLayout.LayoutParams.MATCH_PARENT));
        candidateRegion.addView(candidateHeader, LayoutPolicy.matchWidthHeightPx(
            s.pixels(READING_ROW_DP)));
    }

    /** 历史由编辑器持有；仅发送一次快捷键，不采集文本，也不以投递成功冒充已撤销。 */
    private void performHistory(boolean redo) {
        InputConnection target = s.connection;
        if (target == null || EditorPolicy.password(s.editorInputType)) return;
        if (s.session != 0 && s.hasEngineComposition()) s.command(2);
        if (s.connection != target) return;
        try {
            target.finishComposingText();
        } catch (RuntimeException ignored) { return; }
        s.selectionEcho.invalidate();
        int meta = KeyEvent.META_CTRL_ON | KeyEvent.META_CTRL_LEFT_ON;
        if (redo) meta |= KeyEvent.META_SHIFT_ON | KeyEvent.META_SHIFT_LEFT_ON;
        long down = SystemClock.uptimeMillis();
        // 无论按下是否被接收都尝试松开，连接可在任意一步断开；不追加另一种撤销命令。
        for (int action : new int[] {KeyEvent.ACTION_DOWN, KeyEvent.ACTION_UP})
            InputConnectionGuard.sendKeyEvent(target, new KeyEvent(down,
                SystemClock.uptimeMillis(), action, KeyEvent.KEYCODE_Z, 0, meta,
                KeyCharacterMap.VIRTUAL_KEYBOARD, 0,
                KeyEvent.FLAG_SOFT_KEYBOARD | KeyEvent.FLAG_KEEP_TOUCH_MODE));
    }

    private void bindToolbarAction(Button button, Runnable action) {
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            action.run();
        });
    }

    private TextView toolbarText(float sizeSp) {
        TextView text = ViewPolicy.textLabel(s, null, sizeSp);
        KeyboardTypographyPolicy.setKeyTextSize(text, sizeSp);
        ViewPolicy.clearFontPadding(text);
        return text;
    }


    /** 候选那一行：候选滚动区占满剩余宽度，右端是分隔线加展开键。 */
    void addCandidateLine(LinearLayout candidateRegion, FrameLayout viewport, int height) {
        LinearLayout line = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(line);
        line.addView(viewport, LayoutPolicy.weightedMatchParentParams(1));
        CandidateChevronButton expand = new CandidateChevronButton(s);
        s.expandCandidates = expand;
        expand.setContentDescription("展开候选");
        bindToolbarAction(expand, () -> {
            if (s.candidatePanelOpen) s.closeCandidatePanel();
            else s.openCandidatePanel();
            expand.setExpanded(s.candidatePanelOpen, true);
            s.render();
        });
        ViewPolicy.hide(expand);
        line.addView(expand, LayoutPolicy.linearParamsPx(
            s.pixels(CandidateChevronButton.WIDTH_DP), s.pixels(CandidateChevronButton.BUTTON_DP)));
        s.candidateLine = line;
        ViewPolicy.hide(line);
        candidateRegion.addView(line, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, height));
    }

    /** 工具栏按钮的点按：它的面板开着就回到键盘，否则先关掉别的面板再打开它。 */
    Runnable panelToggle(java.util.function.Supplier<View> panel, Runnable show) {
        return () -> {
            View current = panel.get();
            boolean open = ViewPolicy.isVisible(current);
            s.closeToolbarPanels();
            if (!open) show.run();
            s.render();
        };
    }

    /**
     * 「最近复制」（#5692）：刚复制的文字占工具栏那一行，中间是文字本身（点按粘贴），左边是剪贴板历史入口，右边是关闭和收起键盘。和工具栏同高，替换它显示，过了显示窗口、用过、关掉或开始打字后换回工具栏。
     *
     * <p>剪贴板入口和收起键是工具栏上那两个按钮的同款：只放文字和 × 时，复制之后的一分钟里要打开剪贴板历史或收起键盘，都得先点 × 把这一条永久关掉。
     */
    void addRecentClipRow(LinearLayout candidateRegion) {
        LinearLayout row = LayoutPolicy.row(s);
        ViewPolicy.setCenteredVertically(row);
        LayoutPolicy.setPaddingDp(row, s, 2, 4, 2, 4);
        row.setContentDescription("最近复制");
        Button history = s.shortcutButton(row, "剪贴板", KeyboardShortcutIconPolicy.Icon.CLIPBOARD,
            panelToggle(() -> s.clipboardSurface, s.imePanels::showClipboardHistory));
        history.setContentDescription("剪贴板");
        history.setLayoutParams(LayoutPolicy.linearParamsPx(s.pixels(44), LinearLayout.LayoutParams.MATCH_PARENT));
        recentClipHistoryButton = history;
        KeyboardPressButton paste = ViewPolicy.newPressButton(s);
        paste.setKeyboardRole(KeyboardKeyRole.KEY);
        ViewPolicy.setAllCapsFalse(paste);
        ViewPolicy.setStartCenteredTextSizeSp(paste, 15);
        ViewPolicy.setMaxLinesEllipsized(paste, 1);
        LayoutPolicy.setSymmetricPaddingDp(paste, s, 12, 0);
        ViewPolicy.clearMinimumSize(paste);
        ViewPolicy.clearStateListAnimator(paste);
        bindToolbarAction(paste, s::pasteRecentClip);
        row.addView(paste, LayoutPolicy.weightedMatchParentParams(1));
        KeyboardPressButton dismiss = ViewPolicy.newPressButton(s);
        dismiss.setKeyboardRole(KeyboardKeyRole.GLYPH);
        ViewPolicy.setAllCapsFalse(dismiss);
        dismiss.setText("×");
        KeyboardTypographyPolicy.setKeyTextSize(dismiss, 18);
        ViewPolicy.clearMinimumSize(dismiss);
        ViewPolicy.clearPadding(dismiss);
        dismiss.setContentDescription("不再显示这条复制的内容");
        bindToolbarAction(dismiss, s::dismissRecentClip);
        row.addView(dismiss, LayoutPolicy.linearParamsPx(s.pixels(44), LinearLayout.LayoutParams.MATCH_PARENT));
        // 文本编辑面板也显示最近复制；收起键与普通工具栏一样先返回键盘。
        Button collapse = s.shortcutButton(row, "收起", KeyboardShortcutIconPolicy.Icon.DISMISS,
            () -> {
                if (s.anyToolbarPanelOpen()) {
                    s.closeToolbarPanels();
                    s.render();
                } else s.requestHideSelf(0);
            });
        collapse.setContentDescription("收起键盘");
        collapse.setLayoutParams(LayoutPolicy.linearParamsPx(s.pixels(44), LinearLayout.LayoutParams.MATCH_PARENT));
        recentClipCollapseButton = collapse;
        ViewPolicy.hide(row);
        s.recentClipRow = row;
        s.recentClipButton = paste;
        candidateRegion.addView(row, LayoutPolicy.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.DESIGN_TOOLBAR_ROW_HEIGHT_DP)));
    }

    /** 有内容时显示「最近复制」并换上它的预览，没有时藏起来。`image` 为真时这一条是能发到当前输入框的图片（#6781），按钮写「发送复制的图片」，不显示缩略图：键盘在用户点发送之前不打开图片。 */
    void updateRecentClipRow(String text, boolean image) {
        if (s.recentClipRow == null || s.recentClipButton == null) return;
        boolean visible = text != null || image;
        ViewPolicy.setVisible(s.recentClipRow, visible);
        if (!visible) return;
        recentClipCollapseButton.setContentDescription(s.anyToolbarPanelOpen() ? "返回键盘" : "收起键盘");
        String preview = image ? ClipboardImagePolicy.LABEL : RecentClipboardSuggestion.preview(text);
        if (!preview.contentEquals(s.recentClipButton.getText())) s.recentClipButton.setText(preview);
        s.recentClipButton.setContentDescription(image ? "发送最近复制的图片" : "粘贴最近复制的内容：" + preview);
    }

    /** 内联键盘高度条：调整时替换整行工具栏。 */
    void addInlineHeightBar(LinearLayout candidateRegion) {
        InlineHeightBar bar = new InlineHeightBar(s);
        ViewPolicy.hide(bar);
        bar.setBasePixels(s.pixels(KeyboardHeightPolicy.HEIGHT_PERCENT_BASE_DP));
        s.inlineHeightBar = bar;
        candidateRegion.addView(bar, LayoutPolicy.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.DESIGN_TOOLBAR_ROW_HEIGHT_DP)));
    }

    /** 每次 render 末尾：工具栏图标色、激活底、品牌键配色、展开键与高度条的颜色。 */
    void styleTopRow() {
        applyShortcutOrder();
        KeyboardSkin skin = s.skin;
        String cacheKey = skin.key() + ":" + s.imeStyler.appThemeSeed();
        if (!cacheKey.equals(styleCacheKey)) {
            styleCacheKey = cacheKey;
            iconColor = Color.parseColor(skin.toolbarIcon());
            activeIconColor = Color.parseColor(skin.toolbarActiveIcon());
            activeBackgroundColor = Color.parseColor(skin.toolbarActiveBackground());
            foregroundColor = Color.parseColor(skin.keyForeground());
            hairlineColor = Color.parseColor(skin.hairline());
            hintColor = Color.parseColor(skin.hint());
            returnBackgroundColor = Color.parseColor(skin.returnBackground());
            returnForegroundColor = Color.parseColor(skin.returnForeground());
        }
        for (Button button : shortcutButtons == null ? new Button[0] : shortcutButtons) {
            if (button instanceof KeyboardShortcutButton shortcut) {
                shortcut.setActiveFill(activeBackgroundColor);
                shortcut.setIconColors(iconColor, activeIconColor);
            }
        }
        if (s.dismissShortcutButton instanceof KeyboardShortcutButton dismiss)
            dismiss.setIconColors(foregroundColor, foregroundColor);
        if (recentClipHistoryButton instanceof KeyboardShortcutButton history) {
            history.setActiveFill(activeBackgroundColor);
            history.setIconColors(iconColor, activeIconColor);
        }
        if (recentClipCollapseButton instanceof KeyboardShortcutButton collapse)
            collapse.setIconColors(foregroundColor, foregroundColor);
        if (s.moreButton instanceof KeyboardBrandButton brand) {
            // 品牌标记与工具栏共用皮肤的图标色，面板打开时也采用相同的选中色。
            brand.setLogoColor(s.anyToolbarPanelOpen() ? activeIconColor : iconColor);
            brand.setPanelOpen(s.anyToolbarPanelOpen(), activeBackgroundColor);
        }
        if (s.expandCandidates instanceof CandidateChevronButton chevron) {
            chevron.setColors(s.candidateAppearance.externalSkin() ? s.candidateAppearance.text() : foregroundColor,
                s.candidateAppearance.externalSkin() ? s.candidateAppearance.border() : hairlineColor);
            chevron.setExpanded(s.candidatePanelOpen, true);
        }
        if (s.inlineHeightBar != null)
            s.inlineHeightBar.setColors(foregroundColor, hintColor,
                returnBackgroundColor, returnForegroundColor);
        if (s.floatingBar != null) s.floatingBar.setColors(foregroundColor);
        s.imeBottomBar.style(iconColor, activeIconColor, activeBackgroundColor);
        if (s.preedit != null) {
            ViewPolicy.setTextColor(s.preedit, s.candidateAppearance.externalSkin()
                && ViewPolicy.isVisible(s.candidateViewport) ? s.candidateAppearance.preeditText() : hintColor);
            // 读音的字号只归 ImeStyler.applySkin 管（设置「预编辑字号」按比例缩放，见 ReadingRowPolicy.textSizeSp）。这里原来又写死成 12sp，排在 applySkin 之后，设置怎么改读音都是 12sp（#6107）。
            ViewPolicy.clearBackground(s.preedit);
            ViewPolicy.clearPadding(s.preedit);
            // 读音的最终字号和内边距在这里才定下来（字号按系统字体缩放，最多放大到 1.15 倍，厂商字体的度量也各不相同），读音行的高度要按这一份重新量，见 ReadingRowPolicy。
            s.updateCandidateViewportHeight();
        }
    }

    /** `amount` 份 `color` 混进 `base`（设计的 `mix(color amount, base)`）。 */
    static int mix(int color, int base, float amount) {
        return ColorPolicy.blendOpaque(base, color, amount);
    }
}
