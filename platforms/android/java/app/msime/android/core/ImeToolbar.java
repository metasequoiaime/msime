package app.msime.android;

import android.graphics.Color;
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
    /** 组词时读音那一行的设计高度：12 sp 的读音加一点留白；与候选行合计 56 dp，空闲时的工具栏取同一高度，打字时键盘不变高。系统字体放大或厂商字体更高时按读音实际高度加高，见 ReadingRowPolicy。 */
    static final int READING_ROW_DP = 14;
    /** 候选 chip 那一行的高度：候选字加一行 0.62 倍的释义和选中 chip 的上下留白。设计是 34 dp，实测 34、36 dp 时选中的 chip 和释义都会伸出候选行压到下面的键，所以取 42 dp。 */
    static final int CANDIDATE_LINE_DP = 42;
    /** 多出一行释义时每行加的高度。 */
    static final int EXTRA_GLOSS_ROW_DP = 14;

    private final MSIMEInputService s;
    private Button[] shortcutButtons;
    /** 「最近复制」那一行自己的剪贴板入口和收起键，和工具栏上的同一套配色。 */
    private Button recentClipHistoryButton;
    private Button recentClipCollapseButton;
    /** 最近一次按在读音上的位置（读音视图自己的坐标）。 */
    private float preeditTouchX;
    private float preeditTouchY;
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

    /** 空闲工具栏：品牌、表情、常用语、剪贴板、皮肤、输入方式、浮动键盘、收起，等分整行宽度；哪些显示由 render 按 `touch_toolbar` 与本地设置决定。数字键面上有算式结果时，品牌键右边多一个结果胶囊（{@link ImeCalculator}）。 */
    void installShortcutBar(Button dismissButton) {
        s.shortcutBar.removeAllViews();
        s.dismissShortcutButton = dismissButton;
        Button[] buttons = {s.moreButton, s.emojiShortcutButton, s.phraseShortcutButton,
            s.clipboardShortcutButton, s.skinButton, s.schemeButton, s.floatingShortcutButton, dismissButton};
        shortcutButtons = new Button[] {s.emojiShortcutButton, s.phraseShortcutButton,
            s.clipboardShortcutButton, s.skinButton, s.schemeButton, s.floatingShortcutButton};
        for (Button button : buttons) {
            if (button.getParent() instanceof LinearLayout parent) parent.removeView(button);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                0, s.pixels(44), 1);
            params.setMarginStart(s.pixels(1));
            params.setMarginEnd(s.pixels(1));
            s.shortcutBar.addView(button, params);
            ViewPolicy.setMinimumWidth(button, s.pixels(40));
        }
        // 数字键面的计算结果（#5688）挨着品牌键，在工具栏最左边；没有结果时隐藏，不占宽度。
        Button result = s.imeCalculator.chip();
        if (result.getParent() instanceof LinearLayout parent) parent.removeView(result);
        s.shortcutBar.addView(result, 1, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, s.pixels(44)));
        // 旧的回复、语音、简繁、AI 润色、⚙ 入口不在新工具栏上：AI 在功能面板第 2 页，语音由长按空格进入，设置在功能面板里。按钮对象保留，服务里其余代码照常更新它们的状态。
        for (Button retired : new Button[] {s.scriptShortcutButton, s.aiPolishShortcutButton,
                s.replyShortcutButton, s.voiceShortcutButton, s.layoutSettingsButton}) {
            if (retired == null) continue;
            if (retired.getParent() instanceof LinearLayout parent) parent.removeView(retired);
            ViewPolicy.hide(retired);
        }
    }

    /** 读音行（读音、提示、页码、漢、退出本地模式）与工具栏的滚动容器；读音行只在组词或有提示时显示。 */
    void buildCandidateHeader(LinearLayout candidateRegion) {
        LinearLayout candidateHeader = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(candidateHeader);
        KeyboardGeometry.setPaddingDp(candidateHeader, s, 10, 0, 6, 0);
        s.candidateHeader = candidateHeader;
        s.preedit = toolbarText(12);
        ViewPolicy.setMaxLinesEllipsized(s.preedit, 1);
        // 胶囊紧挨着候选栏和按键，轻点很容易误触，所以只在长按时打开本地模式菜单。
        s.preedit.setOnLongClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.preedit);
            s.imePanels.showLocalInputMenu();
            return true;
        });
        // 组字时轻点读音把组字光标移到点中的字母前（#5613）；落点在按下时记下，抬手成为一次点击时才换算成光标位置。render 只在组字可以移光标时让它可点。
        s.preedit.setOnTouchListener((view, event) -> {
            if (event.getActionMasked() == android.view.MotionEvent.ACTION_DOWN) {
                preeditTouchX = event.getX();
                preeditTouchY = event.getY();
            }
            return false;
        });
        s.preedit.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.preedit);
            s.movePreeditCaret(s.preedit.getOffsetForPosition(preeditTouchX, preeditTouchY));
        });
        s.preedit.setClickable(false);
        LinearLayout preeditFrame = KeyboardGeometry.row(s);
        ViewPolicy.setStartCenteredVertically(preeditFrame);
        preeditFrame.addView(s.preedit, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        candidateHeader.addView(preeditFrame, KeyboardGeometry.weightedWrapParams(1));
        // 宿主提示通道：正常为空，只有准备中、失败或提示时才有文字。
        s.status = toolbarText(10);
        ViewPolicy.setMaxLinesEllipsized(s.status, 1);
        ViewPolicy.setEndCenteredVertically(s.status);
        KeyboardGeometry.setPaddingDp(s.status, s, 6, 0, 2, 0);
        candidateHeader.addView(s.status, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        s.candidatePage = toolbarText(10);
        candidateHeader.addView(s.candidatePage, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        s.shortcutBar = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(s.shortcutBar);
        s.shortcutBar.setContentDescription("键盘快捷栏");
        KeyboardGeometry.setPaddingDp(s.shortcutBar, s, 2, 0, 2, 0);
        s.shortcutScroll = new HorizontalScrollView(s);
        s.shortcutScroll.setHorizontalScrollBarEnabled(false);
        s.shortcutScroll.setFillViewport(true);
        s.shortcutScroll.addView(s.shortcutBar, new HorizontalScrollView.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.MATCH_PARENT));
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
            panelToggle(() -> s.clipboardScroll, s.imePanels::showClipboardHistory));
        s.clipboardShortcutButton.setContentDescription("剪贴板");
        // 浮动键盘按钮默认不在工具栏上，用户在设置「键盘工具栏」里打开（#5621）。
        s.floatingShortcutButton = s.shortcutButton(s.shortcutBar, "浮动键盘",
            KeyboardShortcutIconPolicy.Icon.FLOATING, s::toggleFloatingKeyboard);
        s.floatingShortcutButton.setContentDescription("浮动键盘");
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
        KeyboardGeometry.setKeyTextSize(s.hanjaButton, 12);
        s.hanjaButton.setContentDescription("转换为汉字");
        ViewPolicy.hide(s.hanjaButton);
        bindToolbarAction(s.hanjaButton,
            () -> s.command(KoreanInputPolicy.CONVERT_HANJA_COMMAND));
        KeyboardGeometry.setHorizontalPaddingDp(s.hanjaButton, s, 8);
        ViewPolicy.clearMinimumHeight(s.hanjaButton);
        candidateHeader.addView(s.hanjaButton, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));
        s.exitLocalModeButton = new KeyboardBorderlessButton(s);
        ViewPolicy.setAllCapsFalse(s.exitLocalModeButton);
        s.exitLocalModeButton.setText("×");
        KeyboardGeometry.setKeyTextSize(s.exitLocalModeButton, 14);
        s.exitLocalModeButton.setContentDescription("退出本地模式");
        ViewPolicy.clearPadding(s.exitLocalModeButton);
        s.imeStyler.styleButton(s.exitLocalModeButton, true);
        bindToolbarAction(s.exitLocalModeButton, () -> s.command(3));
        ViewPolicy.clearMinimumHeight(s.exitLocalModeButton);
        candidateHeader.addView(s.exitLocalModeButton, new LinearLayout.LayoutParams(
            s.pixels(32), LinearLayout.LayoutParams.MATCH_PARENT));
        candidateRegion.addView(candidateHeader, KeyboardGeometry.matchWidthHeightPx(
            s.pixels(READING_ROW_DP)));
    }

    private void bindToolbarAction(Button button, Runnable action) {
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            action.run();
        });
    }

    private TextView toolbarText(float sizeSp) {
        TextView text = ViewPolicy.textLabel(s, null, sizeSp);
        KeyboardGeometry.setKeyTextSize(text, sizeSp);
        ViewPolicy.clearFontPadding(text);
        return text;
    }


    /** 候选那一行：候选滚动区占满剩余宽度，右端是分隔线加展开键。 */
    void addCandidateLine(LinearLayout candidateRegion, FrameLayout viewport, int height) {
        LinearLayout line = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(line);
        line.addView(viewport, KeyboardGeometry.weightedMatchParentParams(1));
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
        line.addView(expand, KeyboardGeometry.linearParamsPx(
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
            boolean open = current != null && current.getVisibility() == View.VISIBLE;
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
        LinearLayout row = KeyboardGeometry.row(s);
        ViewPolicy.setCenteredVertically(row);
        KeyboardGeometry.setPaddingDp(row, s, 2, 4, 2, 4);
        row.setContentDescription("最近复制");
        Button history = s.shortcutButton(row, "剪贴板", KeyboardShortcutIconPolicy.Icon.CLIPBOARD,
            panelToggle(() -> s.clipboardScroll, s.imePanels::showClipboardHistory));
        history.setContentDescription("剪贴板");
        history.setLayoutParams(KeyboardGeometry.linearParamsPx(s.pixels(44), LinearLayout.LayoutParams.MATCH_PARENT));
        recentClipHistoryButton = history;
        KeyboardPressButton paste = ViewPolicy.newPressButton(s);
        paste.setKeyboardRole(KeyboardKeyRole.KEY);
        ViewPolicy.setAllCapsFalse(paste);
        ViewPolicy.setStartCenteredTextSizeSp(paste, 15);
        ViewPolicy.setMaxLinesEllipsized(paste, 1);
        KeyboardGeometry.setSymmetricPaddingDp(paste, s, 12, 0);
        ViewPolicy.clearMinimumSize(paste);
        ViewPolicy.clearStateListAnimator(paste);
        bindToolbarAction(paste, s::pasteRecentClip);
        row.addView(paste, KeyboardGeometry.weightedMatchParentParams(1));
        KeyboardPressButton dismiss = ViewPolicy.newPressButton(s);
        dismiss.setKeyboardRole(KeyboardKeyRole.GLYPH);
        ViewPolicy.setAllCapsFalse(dismiss);
        dismiss.setText("×");
        KeyboardGeometry.setKeyTextSize(dismiss, 18);
        ViewPolicy.clearMinimumSize(dismiss);
        ViewPolicy.clearPadding(dismiss);
        dismiss.setContentDescription("不再显示这条复制的内容");
        bindToolbarAction(dismiss, s::dismissRecentClip);
        row.addView(dismiss, KeyboardGeometry.linearParamsPx(s.pixels(44), LinearLayout.LayoutParams.MATCH_PARENT));
        // 这一行只在没有工具栏面板开着时显示，收起键在这里只有收起键盘这一个意思。
        Button collapse = s.shortcutButton(row, "收起", KeyboardShortcutIconPolicy.Icon.DISMISS,
            () -> s.requestHideSelf(0));
        collapse.setContentDescription("收起键盘");
        collapse.setLayoutParams(KeyboardGeometry.linearParamsPx(s.pixels(44), LinearLayout.LayoutParams.MATCH_PARENT));
        recentClipCollapseButton = collapse;
        ViewPolicy.hide(row);
        s.recentClipRow = row;
        s.recentClipButton = paste;
        candidateRegion.addView(row, KeyboardGeometry.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.DESIGN_TOOLBAR_ROW_HEIGHT_DP)));
    }

    /** 有内容时显示「最近复制」并换上它的预览，没有时藏起来。 */
    void updateRecentClipRow(String text) {
        if (s.recentClipRow == null || s.recentClipButton == null) return;
        boolean visible = text != null;
        ViewPolicy.setVisible(s.recentClipRow, visible);
        if (!visible) return;
        String preview = RecentClipboardSuggestion.preview(text);
        if (!preview.contentEquals(s.recentClipButton.getText())) s.recentClipButton.setText(preview);
        s.recentClipButton.setContentDescription("粘贴最近复制的内容：" + preview);
    }

    /** 内联键盘高度条：调整时替换整行工具栏。 */
    void addInlineHeightBar(LinearLayout candidateRegion) {
        InlineHeightBar bar = new InlineHeightBar(s);
        ViewPolicy.hide(bar);
        bar.setBasePixels(s.pixels(KeyboardGeometry.HEIGHT_PERCENT_BASE_DP));
        s.inlineHeightBar = bar;
        candidateRegion.addView(bar, KeyboardGeometry.matchWidthHeightPx(
            s.pixels(KeyboardGeometry.DESIGN_TOOLBAR_ROW_HEIGHT_DP)));
    }

    /** 每次 render 末尾：工具栏图标色、激活底、品牌键配色、展开键与高度条的颜色。 */
    void styleTopRow() {
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
            // 设计的 logoCirc / logoBg 是应用主题的季节色（与开屏、设置页的 logo 同一套），不是从键盘皮肤的强调色混出来的。
            AppThemePalette palette = AppThemePalette.of(s.imeStyler.appThemeSeed(), skin.dark());
            brand.setLogoColors(palette.logoDisc, palette.logoBackground);
            brand.setPanelOpen(s.anyToolbarPanelOpen(), activeBackgroundColor);
        }
        if (s.expandCandidates instanceof CandidateChevronButton chevron) {
            chevron.setColors(foregroundColor, hairlineColor);
            chevron.setExpanded(s.candidatePanelOpen, true);
        }
        if (s.inlineHeightBar != null)
            s.inlineHeightBar.setColors(foregroundColor, hintColor,
                returnBackgroundColor, returnForegroundColor);
        if (s.floatingBar != null) s.floatingBar.setColors(foregroundColor);
        s.imeBottomBar.style(iconColor, activeIconColor, activeBackgroundColor);
        if (s.preedit != null) {
            ViewPolicy.setTextColor(s.preedit, hintColor);
            KeyboardGeometry.setKeyTextSize(s.preedit, 12);
            ViewPolicy.clearBackground(s.preedit);
            ViewPolicy.clearPadding(s.preedit);
            // 读音的最终字号和内边距在这里才定下来（系统字体放大时 12sp 最多放大到 1.15 倍，厂商字体的度量也各不相同），读音行的高度要按这一份重新量，见 ReadingRowPolicy。
            s.updateCandidateViewportHeight();
        }
    }

    /** `amount` 份 `color` 混进 `base`（设计的 `mix(color amount, base)`）。 */
    static int mix(int color, int base, float amount) {
        float t = KeyboardGeometry.bounded(amount, 0f, 1f);
        return Color.rgb(
            Math.round(Color.red(color) * t + Color.red(base) * (1 - t)),
            Math.round(Color.green(color) * t + Color.green(base) * (1 - t)),
            Math.round(Color.blue(color) * t + Color.blue(base) * (1 - t)));
    }
}
