package app.msime.android;

import android.widget.Button;
import android.widget.LinearLayout;
import java.util.ArrayList;
import java.util.List;

/**
 * 拼音九键 1 键（@#）的常用标点：没在组字时点 1 键，候选条换成当前模式的常用标点，与 26 键逗号键长按的那一组相同（{@link QuickPunctuationPolicy}，中文模式是 ， 。 ？ ！ 、 ； ：），点一个经 {@link MSIMEInputService#type} 写出，和逗号键走同一条路，中英文标点、成对标点照常由 Engine 处理。与 iOS 的 `showNineKeyQuickSymbols` 一致；符号面板改从底行最左边的 符 打开。
 *
 * <p>这些标点只在候选条上停留到下一次 Engine 转换：按任何键都会经过 {@link MSIMEInputService#apply}，在那里收起，候选条随之画 Engine 自己的候选。开始组字、离开拼音九键的字母键面或离开输入框时同样不再显示；点选上屏后收起，候选条回到空闲。
 */
final class ImeNineKeyQuickPunctuation {
    private final MSIMEInputService s;
    private final List<Button> buttons = new ArrayList<>();
    private List<QuickPunctuationPolicy.Entry> entries = List.of();

    ImeNineKeyQuickPunctuation(MSIMEInputService s) {
        this.s = s;
    }

    /** 候选条这时该不该画常用标点：1 键放进来过、还没被 Engine 的转换换掉，并且画着的仍是拼音九键的字母键面、没在组字。 */
    boolean active() {
        return !entries.isEmpty() && !s.hasEngineComposition()
            && s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.imeLetterRows.drawnTouchLayout() == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;
    }

    /** 1 键空闲时的点按：把常用标点放进候选条。 */
    void show() {
        entries = s.quickPunctuationEntries();
        s.render();
    }

    /** Engine 给出新的转换或离开输入框时收起，不重画。 */
    void clear() {
        entries = List.of();
    }

    /** 把标点按钮接到候选行里，样式与英文建议、邮箱后缀相同。 */
    void render(LinearLayout activeCandidates, boolean horizontal) {
        // 键面和写出的字一致：中文标点关着时 Engine 写半角，这里也画半角。
        boolean chinese = s.sendsChinesePunctuation();
        for (int slot = 0; slot < entries.size(); slot++) {
            while (buttons.size() <= slot) buttons.add(makeButton(buttons.size()));
            Button button = buttons.get(slot);
            QuickPunctuationPolicy.Entry entry = entries.get(slot);
            String face = chinese ? entry.face() : String.valueOf(entry.input());
            ViewPolicy.show(button);
            button.setText(face);
            KeyboardTypographyPolicy.setKeyTextSize(button, s.candidateFontSize);
            button.setContentDescription("常用标点 " + (slot + 1) + "：" + face);
            s.imeStyler.styleButton(button, false);
            ViewPolicy.setTypeface(button, s.imeStyler.candidateTypeface());
            activeCandidates.addView(button, new LinearLayout.LayoutParams(
                horizontal ? LinearLayout.LayoutParams.WRAP_CONTENT : LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT));
        }
        for (int slot = entries.size(); slot < buttons.size(); slot++) ViewPolicy.hide(buttons.get(slot));
    }

    private Button makeButton(int slot) {
        Button button = new KeyboardPressButton(s);
        s.bindCandidateRenderRelease(button);
        ViewPolicy.setAllCapsFalse(button);
        ViewPolicy.bindClick(button, () -> {
            s.imeKeyFeedback.playFeedback(button);
            use(slot);
        });
        return button;
    }

    /** 上屏点中的标点：先收起，再像逗号键长按菜单那样经 type() 写出；Engine 不收这个标点、没有转换时也要重画一次，候选条才回到空闲。 */
    private void use(int slot) {
        if (slot < 0 || slot >= entries.size()) return;
        char input = entries.get(slot).input();
        entries = List.of();
        s.type(input);
        s.render();
    }
}
