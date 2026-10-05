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
    /** 组词时读音那一行的高度：12 sp 的读音加上下留白。 */
    static final int READING_ROW_DP = 16;
    /** 候选 chip 那一行的高度（设计 34 dp）。 */
    static final int CANDIDATE_LINE_DP = 34;
    /** 多出一行释义时每行加的高度。 */
    static final int EXTRA_GLOSS_ROW_DP = 14;

    private final MSIMEInputService s;

    ImeToolbar(MSIMEInputService s) {
        this.s = s;
    }

    /** 空闲工具栏：品牌、表情、常用语、剪贴板、皮肤、输入方式、收起，等分整行宽度；哪些显示由 render 按 `touch_toolbar` 决定。 */
    void installShortcutBar(Button dismissButton) {
        s.shortcutBar.removeAllViews();
        s.dismissShortcutButton = dismissButton;
        Button[] buttons = {s.moreButton, s.emojiShortcutButton, s.phraseShortcutButton,
            s.clipboardShortcutButton, s.skinButton, s.schemeButton, dismissButton};
        for (Button button : buttons) {
            if (button.getParent() instanceof LinearLayout parent) parent.removeView(button);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                0, s.pixels(44), 1);
            params.setMarginStart(s.pixels(1));
            params.setMarginEnd(s.pixels(1));
            s.shortcutBar.addView(button, params);
            button.setMinWidth(s.pixels(40));
            button.setMinimumWidth(s.pixels(40));
        }
        // 旧的回复、语音、简繁、AI 润色、⚙ 入口不在新工具栏上：AI 在功能面板第 2 页，语音由长按空格进入，设置在功能面板里。按钮对象保留，服务里其余代码照常更新它们的状态。
        for (Button retired : new Button[] {s.scriptShortcutButton, s.aiPolishShortcutButton,
                s.replyShortcutButton, s.voiceShortcutButton, s.layoutSettingsButton}) {
            if (retired == null) continue;
            if (retired.getParent() instanceof LinearLayout parent) parent.removeView(retired);
            retired.setVisibility(View.GONE);
        }
    }

    /** 读音行（读音、提示、页码、漢、退出本地模式）与工具栏的滚动容器；读音行只在组词或有提示时显示。 */
    void buildCandidateHeader(LinearLayout candidateRegion) {
        LinearLayout candidateHeader = new LinearLayout(s);
        candidateHeader.setGravity(Gravity.CENTER_VERTICAL);
        candidateHeader.setPadding(s.pixels(10), 0, s.pixels(6), 0);
        s.candidateHeader = candidateHeader;
        s.preedit = new TextView(s);
        s.preedit.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        s.preedit.setMaxLines(1);
        s.preedit.setIncludeFontPadding(false);
        s.preedit.setEllipsize(android.text.TextUtils.TruncateAt.END);
        s.preedit.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.preedit);
            s.imePanels.showLocalInputMenu();
        });
        LinearLayout preeditFrame = new LinearLayout(s);
        preeditFrame.setGravity(Gravity.CENTER_VERTICAL | Gravity.START);
        preeditFrame.addView(s.preedit, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        candidateHeader.addView(preeditFrame, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        // 宿主提示通道：正常为空，只有准备中、失败或提示时才有文字。
        s.status = new TextView(s);
        s.status.setTextSize(TypedValue.COMPLEX_UNIT_SP, 10);
        s.status.setMaxLines(1);
        s.status.setIncludeFontPadding(false);
        s.status.setEllipsize(android.text.TextUtils.TruncateAt.END);
        s.status.setGravity(Gravity.CENTER_VERTICAL | Gravity.END);
        s.status.setPadding(s.pixels(6), 0, s.pixels(2), 0);
        candidateHeader.addView(s.status, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        s.candidatePage = new TextView(s);
        s.candidatePage.setTextSize(TypedValue.COMPLEX_UNIT_SP, 10);
        s.candidatePage.setIncludeFontPadding(false);
        candidateHeader.addView(s.candidatePage, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        s.shortcutBar = new LinearLayout(s);
        s.shortcutBar.setOrientation(LinearLayout.HORIZONTAL);
        s.shortcutBar.setGravity(Gravity.CENTER_VERTICAL);
        s.shortcutBar.setContentDescription("键盘快捷栏");
        s.shortcutBar.setPadding(s.pixels(2), 0, s.pixels(2), 0);
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
        s.hanjaButton.setAllCaps(false);
        s.hanjaButton.setText("漢");
        s.hanjaButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        s.hanjaButton.setContentDescription("转换为汉字");
        s.hanjaButton.setVisibility(View.GONE);
        s.hanjaButton.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.hanjaButton);
            s.command(KoreanInputPolicy.CONVERT_HANJA_COMMAND);
        });
        s.hanjaButton.setPadding(s.pixels(8), 0, s.pixels(8), 0);
        s.hanjaButton.setMinHeight(0);
        s.hanjaButton.setMinimumHeight(0);
        candidateHeader.addView(s.hanjaButton, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));
        s.exitLocalModeButton = new KeyboardBorderlessButton(s);
        s.exitLocalModeButton.setAllCaps(false);
        s.exitLocalModeButton.setText("×");
        s.exitLocalModeButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        s.exitLocalModeButton.setContentDescription("退出本地模式");
        s.exitLocalModeButton.setPadding(0, 0, 0, 0);
        s.imeStyler.styleButton(s.exitLocalModeButton, true);
        s.exitLocalModeButton.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.exitLocalModeButton);
            s.command(3);
        });
        s.exitLocalModeButton.setMinHeight(0);
        s.exitLocalModeButton.setMinimumHeight(0);
        candidateHeader.addView(s.exitLocalModeButton, new LinearLayout.LayoutParams(
            s.pixels(32), LinearLayout.LayoutParams.MATCH_PARENT));
        candidateRegion.addView(candidateHeader, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(READING_ROW_DP)));
    }

    /** 候选那一行：候选滚动区占满剩余宽度，右端是分隔线加展开键。 */
    void addCandidateLine(LinearLayout candidateRegion, FrameLayout viewport, int height) {
        LinearLayout line = new LinearLayout(s);
        line.setOrientation(LinearLayout.HORIZONTAL);
        line.setGravity(Gravity.CENTER_VERTICAL);
        line.addView(viewport, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 1));
        CandidateChevronButton expand = new CandidateChevronButton(s);
        s.expandCandidates = expand;
        expand.setContentDescription("展开候选");
        expand.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(expand);
            if (s.candidatePanelOpen) s.closeCandidatePanel();
            else s.openCandidatePanel();
            expand.setExpanded(s.candidatePanelOpen, true);
            s.render();
        });
        expand.setVisibility(View.GONE);
        line.addView(expand, new LinearLayout.LayoutParams(
            s.pixels(CandidateChevronButton.WIDTH_DP), s.pixels(CandidateChevronButton.BUTTON_DP)));
        s.candidateLine = line;
        line.setVisibility(View.GONE);
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

    /** 内联键盘高度条：调整时替换整行工具栏。 */
    void addInlineHeightBar(LinearLayout candidateRegion) {
        InlineHeightBar bar = new InlineHeightBar(s);
        bar.setVisibility(View.GONE);
        bar.setBasePixels(s.pixels(KeyboardGeometry.HEIGHT_PERCENT_BASE_DP));
        s.inlineHeightBar = bar;
        candidateRegion.addView(bar, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            s.pixels(KeyboardGeometry.DESIGN_TOOLBAR_ROW_HEIGHT_DP)));
    }

    /** 每次 render 末尾：工具栏图标色、激活底、品牌键配色、展开键与高度条的颜色。 */
    void styleTopRow() {
        KeyboardSkin skin = s.skin;
        int icon = Color.parseColor(skin.toolbarIcon());
        int active = Color.parseColor(skin.toolbarActiveIcon());
        int soft = Color.parseColor(skin.toolbarActiveBackground());
        int fg = Color.parseColor(skin.keyForeground());
        for (Button button : new Button[] {s.emojiShortcutButton, s.phraseShortcutButton,
                s.clipboardShortcutButton, s.skinButton, s.schemeButton}) {
            if (button instanceof KeyboardShortcutButton shortcut) {
                shortcut.setActiveFill(soft);
                shortcut.setIconColors(icon, active);
            }
        }
        if (s.dismissShortcutButton instanceof KeyboardShortcutButton dismiss)
            dismiss.setIconColors(fg, fg);
        if (s.moreButton instanceof KeyboardBrandButton brand) {
            // 设计的 logoCirc / logoBg 是应用主题的季节色（与开屏、设置页的 logo 同一套），不是从键盘皮肤的强调色混出来的。
            AppThemePalette palette = AppThemePalette.of(s.imeStyler.appThemeSeed(), skin.dark());
            brand.setLogoColors(palette.logoDisc, palette.logoBackground);
            brand.setPanelOpen(s.anyToolbarPanelOpen(), soft);
        }
        if (s.expandCandidates instanceof CandidateChevronButton chevron) {
            chevron.setColors(fg, Color.parseColor(skin.hairline()));
            chevron.setExpanded(s.candidatePanelOpen, true);
        }
        if (s.inlineHeightBar != null)
            s.inlineHeightBar.setColors(fg, Color.parseColor(skin.hint()),
                Color.parseColor(skin.returnBackground()), Color.parseColor(skin.returnForeground()));
        if (s.preedit != null) {
            s.preedit.setTextColor(Color.parseColor(skin.hint()));
            s.preedit.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
            s.preedit.setBackground(null);
            s.preedit.setPadding(0, 0, 0, 0);
        }
    }

    /** `amount` 份 `color` 混进 `base`（设计的 `mix(color amount, base)`）。 */
    static int mix(int color, int base, float amount) {
        float t = Math.max(0f, Math.min(1f, amount));
        return Color.rgb(
            Math.round(Color.red(color) * t + Color.red(base) * (1 - t)),
            Math.round(Color.green(color) * t + Color.green(base) * (1 - t)),
            Math.round(Color.blue(color) * t + Color.blue(base) * (1 - t)));
    }
}
