package app.msime.android;

import android.graphics.Color;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.TextView;

/** 键盘顶部：候选头（品牌标、读音、提示、漢、展开、退出本地模式）与空闲时的快捷栏；从 MSIMEInputService 原样搬出，状态仍在服务里。 */
final class ImeToolbar {
    private final MSIMEInputService s;

    ImeToolbar(MSIMEInputService s) {
        this.s = s;
    }

    void installShortcutBar(Button dismissButton) {
        s.shortcutBar.removeAllViews();
        // The design's idle row: the brand mark first, then the scheme pill, the content tools and 收起, with ⚙ at the far end.
        Button[] buttons = {s.moreButton, s.schemeButton, s.replyShortcutButton, s.emojiShortcutButton,
            s.voiceShortcutButton, s.skinButton, dismissButton, s.layoutSettingsButton};
        for (Button button : buttons) {
            if (button.getParent() instanceof LinearLayout parent) parent.removeView(button);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                0, s.pixels(44), 1);
            params.setMarginStart(s.pixels(2));
            params.setMarginEnd(s.pixels(2));
            s.shortcutBar.addView(button, params);
            button.setMinWidth(s.pixels(44));
            button.setMinimumWidth(s.pixels(44));
        }
        // Traditional output and AI are persistent settings/actions in the Apple layout; keep
        // their Android controls detached from the shortcut strip rather than duplicating them.
        s.scriptShortcutButton.setVisibility(View.GONE);
        s.aiPolishShortcutButton.setVisibility(View.GONE);
    }

    void buildCandidateHeader(LinearLayout candidateRegion) {
        LinearLayout candidateHeader = new LinearLayout(s);
        candidateHeader.setGravity(Gravity.CENTER_VERTICAL);
        candidateHeader.setPadding(s.pixels(10), s.pixels(6), s.pixels(6), s.pixels(2));
        // The brand mark leads the header the way it leads the macOS candidate window's top row: 16dp, then a 6dp gap before the reading. The header is never hidden, so the mark is always present; it is decorative, since the idle pill beside it already reads 水杉输入法.
        s.candidateBrandMark = new KeyboardBrandMark(s, () -> Color.parseColor(s.skin.accent()));
        LinearLayout.LayoutParams brandMarkLayout = new LinearLayout.LayoutParams(
            s.pixels(16), s.pixels(16));
        brandMarkLayout.setMarginEnd(s.pixels(6));
        candidateHeader.addView(s.candidateBrandMark, brandMarkLayout);
        s.preedit = new TextView(s);
        s.preedit.setTextSize(TypedValue.COMPLEX_UNIT_SP, s.candidatePreeditFontSize);
        s.preedit.setMaxLines(1);
        s.preedit.setEllipsize(android.text.TextUtils.TruncateAt.END);
        s.preedit.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.preedit);
            s.imePanels.showLocalInputMenu();
        });
        // The pill hugs its own text, so it needs a parent that bounds it: a weighted TextView would
        // stretch the outline the whole width of the keyboard.
        LinearLayout preeditFrame = new LinearLayout(s);
        preeditFrame.setGravity(Gravity.CENTER_VERTICAL | Gravity.START);
        preeditFrame.addView(s.preedit, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        candidateHeader.addView(preeditFrame, new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        // The host notice channel: idle it names the build, and a deferred or failed preference load
        // is the only thing the user ever reads here. It keeps the caption weight the design gives a
        // secondary label rather than the headline it used to be at the top of the keyboard.
        s.status = new TextView(s);
        s.status.setTextSize(TypedValue.COMPLEX_UNIT_SP, 10);
        s.status.setMaxLines(1);
        s.status.setEllipsize(android.text.TextUtils.TruncateAt.END);
        s.status.setGravity(Gravity.CENTER_VERTICAL | Gravity.END);
        s.status.setPadding(s.pixels(6), 0, s.pixels(2), 0);
        candidateHeader.addView(s.status, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        s.candidatePage = new TextView(s);
        candidateHeader.addView(s.candidatePage, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        s.shortcutBar = new LinearLayout(s);
        s.shortcutBar.setOrientation(LinearLayout.HORIZONTAL);
        s.shortcutBar.setGravity(Gravity.CENTER_VERTICAL);
        s.shortcutBar.setContentDescription("键盘快捷栏");
        s.shortcutBar.setPadding(s.pixels(8), 0, s.pixels(8), 0);
        s.shortcutScroll = new HorizontalScrollView(s);
        s.shortcutScroll.setHorizontalScrollBarEnabled(false);
        s.shortcutScroll.setContentDescription("键盘快捷栏");
        s.shortcutScroll.setFillViewport(true);
        // The glyphs share the width evenly instead of queueing from the left edge; the scroll view
        // stays as the fallback for a narrow screen that cannot give each a 44dp target.
        s.shortcutScroll.addView(s.shortcutBar, new HorizontalScrollView.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.MATCH_PARENT));
        s.scriptShortcutButton = s.button(s.shortcutBar, "简", s::toggleChineseOutput);
        s.scriptShortcutButton.setContentDescription("切换到繁体");
        s.emojiShortcutButton = s.shortcutButton(s.shortcutBar, "☺",
            KeyboardShortcutIconPolicy.Icon.EMOJI, s.imePanels::showEmojiPicker);
        s.emojiShortcutButton.setContentDescription("打开表情浏览");
        s.keyId(s.emojiShortcutButton, "SoftEmoji");
        s.voiceShortcutButton = s.shortcutButton(s.shortcutBar, "语音",
            KeyboardShortcutIconPolicy.Icon.VOICE, s::showVoiceResult);
        s.voiceShortcutButton.setContentDescription("打开语音结果");
        s.keyId(s.voiceShortcutButton, "SoftVoice");
        s.aiPolishShortcutButton = s.button(s.shortcutBar, "AI", s.imePanels::showAiPolish);
        s.aiPolishShortcutButton.setContentDescription("打开 AI 润色");
        s.replyShortcutButton = s.shortcutButton(s.shortcutBar, "回复",
            KeyboardShortcutIconPolicy.Icon.REPLY, s::toggleReplyKeyboard);
        s.replyShortcutButton.setContentDescription("生成高情商回复");
        // 漢 is the touch counterpart of a Korean keyboard's Hanja key: it lists the Hanja of the composing syllable on the strip below and closes the list again. It sits in the header so it stays put while the list fills the strip, and render() shows it only while a Korean syllable composes; the filled face says the list is open. While a Zhuyin conversion composes the same key reads 選 and opens the conversion's list, through the same shared command 16 (MSIME_OPEN_CANDIDATE_LIST).
        KeyboardPressButton hanja = new KeyboardPressButton(s);
        hanja.setKeyboardRole(KeyboardKeyRole.GLYPH);
        s.hanjaButton = hanja;
        s.hanjaButton.setAllCaps(false);
        s.hanjaButton.setText("漢");
        s.hanjaButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 16);
        s.hanjaButton.setContentDescription("转换为汉字");
        s.hanjaButton.setVisibility(View.GONE);
        s.hanjaButton.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.hanjaButton);
            s.command(KoreanInputPolicy.CONVERT_HANJA_COMMAND);
        });
        s.hanjaButton.setMinHeight(0);
        s.hanjaButton.setMinimumHeight(0);
        candidateHeader.addView(s.hanjaButton, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));
        KeyboardPressButton expand = new KeyboardPressButton(s);
        expand.setKeyboardRole(KeyboardKeyRole.GLYPH);
        s.expandCandidates = expand;
        s.expandCandidates.setAllCaps(false);
        s.expandCandidates.setText("展开");
        s.expandCandidates.setTextSize(TypedValue.COMPLEX_UNIT_SP, 13);
        s.expandCandidates.setContentDescription("展开候选面板");
        s.expandCandidates.setOnClickListener(ignored -> {
            s.imeKeyFeedback.playFeedback(s.expandCandidates);
            s.openCandidatePanel();
        });
        s.expandCandidates.setMinHeight(0);
        s.expandCandidates.setMinimumHeight(0);
        s.expandCandidates.setPadding(s.pixels(10), 0, s.pixels(10), 0);
        candidateHeader.addView(s.expandCandidates, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.MATCH_PARENT));
        s.exitLocalModeButton = new KeyboardBorderlessButton(s);
        s.exitLocalModeButton.setAllCaps(false);
        s.exitLocalModeButton.setText("×");
        s.exitLocalModeButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 22);
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
            s.pixels(40), LinearLayout.LayoutParams.MATCH_PARENT));
        // 标题行固定高度：空闲时只有一个小标签，组词时出现「展开」等按钮，按内容撑高会让键盘在打字时变高。
        candidateRegion.addView(candidateHeader, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, s.pixels(MSIMEInputService.CANDIDATE_HEADER_HEIGHT_DP)));
    }
}
