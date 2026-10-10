package app.msime.android;

import android.widget.Button;
import android.widget.LinearLayout;
import java.util.ArrayList;
import java.util.List;

/**
 * 邮箱后缀建议（#6147）：在系统标记为邮箱的输入框里，光标前打到 `xxx@`（或 `xxx@` 后面接着打了域名的开头）时，候选栏列出 {@link EmailSuffixPolicy#SUFFIXES} 里能接上的后缀，点一下补全。
 *
 * <p>只在 `EMAIL_ADDRESS` / `WEB_EMAIL_ADDRESS` 输入框里出现，聊天框、搜索框里的 `@` 不触发，也没有开关。上文在每次选区变化时向编辑器要 {@link EmailSuffixPolicy#MAX_CONTEXT} 个字符；组字中不读。读到的文字只在内存里匹配一次，不记录、不保存，与 {@link ImeCalculator} 的约定相同。后缀按 ASCII 原样上屏，不随全角模式变成全角。
 */
final class ImeEmailSuffixes {
    private final MSIMEInputService s;
    private final List<Button> buttons = new ArrayList<>();
    private EmailSuffixPolicy.Match match;

    ImeEmailSuffixes(MSIMEInputService s) {
        this.s = s;
    }

    /** 候选栏这时该不该显示后缀：有匹配且没在组字（组字时候选栏归引擎的候选）。 */
    boolean active() {
        return match != null && !s.hasEngineComposition();
    }

    /** 按光标前的文字重新匹配；结果变了才重画。 */
    void refresh() {
        EmailSuffixPolicy.Match next = null;
        if (s.connection != null && EditorPolicy.emailAddress(s.editorInputType) && !s.hasEngineComposition()) {
            CharSequence before = s.connection.getTextBeforeCursor(EmailSuffixPolicy.MAX_CONTEXT, 0);
            next = EmailSuffixPolicy.match(before);
        }
        if (java.util.Objects.equals(next, match)) return;
        match = next;
        s.render();
    }

    /** 离开输入框时收起，不重画。 */
    void clear() {
        match = null;
    }

    /** 把后缀按钮接到候选行里，样式与英文建议相同。 */
    void render(LinearLayout activeCandidates) {
        List<String> suffixes = match == null ? List.of() : match.suffixes();
        for (int slot = 0; slot < suffixes.size(); slot++) {
            while (buttons.size() <= slot) buttons.add(makeButton(buttons.size()));
            Button button = buttons.get(slot);
            String suffix = suffixes.get(slot);
            ViewPolicy.show(button);
            button.setText(suffix);
            KeyboardTypographyPolicy.setKeyTextSize(button, s.candidateFontSize);
            button.setContentDescription("邮箱后缀 " + (slot + 1) + "：" + suffix);
            s.imeStyler.styleButton(button, false);
            ViewPolicy.setTypeface(button, s.imeStyler.candidateTypeface());
            activeCandidates.addView(button, new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        for (int slot = suffixes.size(); slot < buttons.size(); slot++) ViewPolicy.hide(buttons.get(slot));
    }

    private Button makeButton(int slot) {
        Button button = new KeyboardPressButton(s);
        ViewPolicy.setAllCapsFalse(button);
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            use(slot);
        });
        return button;
    }

    /** 补全：按点下这一刻的上文重新匹配，删掉已经打的域名，上屏完整后缀。上文已经变了、对不上这个后缀时只收起。 */
    private void use(int slot) {
        EmailSuffixPolicy.Match shown = match;
        if (shown == null || slot < 0 || slot >= shown.suffixes().size() || s.connection == null) return;
        String suffix = shown.suffixes().get(slot);
        CharSequence before = s.connection.getTextBeforeCursor(EmailSuffixPolicy.MAX_CONTEXT, 0);
        EmailSuffixPolicy.Replacement replacement = EmailSuffixPolicy.replacement(
            EmailSuffixPolicy.match(before), suffix);
        if (replacement != null) {
            s.connection.beginBatchEdit();
            try {
                if (replacement.deleteCount() == 0 || s.deleteBeforeCursor(replacement.deleteCount()))
                    s.commitText(replacement.insert());
            } finally {
                s.connection.endBatchEdit();
            }
        }
        // 补全后上文以完整后缀结尾，不再匹配；编辑器随后的选区回报也会再匹配一次。
        match = null;
        s.render();
    }
}
