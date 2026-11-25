package app.msime.android;

import android.text.SpannableStringBuilder;
import android.view.Gravity;
import android.view.View;
import android.widget.FrameLayout;
import android.widget.TextView;
import java.util.List;

/**
 * 展开面板（普通网格和九键三栏）开着时，云/AI 候选等着没换上的提示（{@link CandidatePanelCue}）。
 *
 * <p>和其他面板一样是键盘表面上的覆盖层，顶边对齐到顶部一行下面再空一点，水平居中：落在网格第一行或九键候选栏的上方，不占格子的位置。按皮肤的选中候选底色和字色画，来源标记沿用候选上的图形。
 */
final class ImeCandidatePanelCue {
    private static final int TOP_GAP_DP = 6;
    private static final float TEXT_SP = 13;
    private final MSIMEInputService s;
    private TextView pill;
    private String shown = "";

    ImeCandidatePanelCue(MSIMEInputService s) {
        this.s = s;
    }

    View build() {
        pill = ViewPolicy.centeredText(s, CandidatePanelCue.LABEL, TEXT_SP);
        ViewPolicy.setSingleLine(pill);
        pill.setPadding(s.pixels(12), s.pixels(4), s.pixels(12), s.pixels(4));
        pill.setMinHeight(s.pixels(32));
        ViewPolicy.bindClick(pill, () -> {
            s.imeKeyFeedback.playFeedback(pill);
            s.releaseHeldCandidates();
        });
        ViewPolicy.hide(pill);
        shown = "";
        return pill;
    }

    FrameLayout.LayoutParams layoutParams() {
        return new FrameLayout.LayoutParams(FrameLayout.LayoutParams.WRAP_CONTENT,
            FrameLayout.LayoutParams.WRAP_CONTENT, Gravity.TOP | Gravity.CENTER_HORIZONTAL);
    }

    void alignBelow(int top) {
        if (pill != null && pill.getLayoutParams() instanceof FrameLayout.LayoutParams params
                && params.topMargin != top + s.pixels(TOP_GAP_DP)) {
            params.topMargin = top + s.pixels(TOP_GAP_DP);
            pill.post(() -> pill.setLayoutParams(params));
        }
    }

    /** 面板开着且有在等的云/AI 候选时显示，否则藏起来。每次 render 和在线结果排进来、放行时调用。 */
    void sync() {
        if (pill == null) return;
        List<Integer> sources = s.candidatePanelOpen
            ? CandidatePanelCue.sources(s.heldPanelSources()) : List.of();
        String description = CandidatePanelCue.description(sources);
        if (description.isEmpty()) {
            ViewPolicy.setVisibleIfChanged(pill, false);
            shown = "";
            return;
        }
        CandidateAppearance.Palette palette = s.candidateAppearance;
        String key = description + "|" + palette.chip() + "|" + palette.chipText() + "|" + palette.surface();
        if (!key.equals(shown)) {
            shown = key;
            CharSequence label = CandidatePanelCue.LABEL;
            for (int i = sources.size() - 1; i >= 0; i--)
                label = CandidateSourceMarkSpan.insert(s, label, 0, s.candidateSourceGlyph(sources.get(i)),
                    palette.chipText());
            pill.setText(new SpannableStringBuilder(label));
            pill.setTextColor(palette.chipText());
            // 有的皮肤选中候选不垫底色：提示浮在格子上面，没有底色就和候选字混在一起，改垫候选区的底色。
            int fill = android.graphics.Color.alpha(palette.chip()) == 0 ? palette.surface() : palette.chip();
            ViewPolicy.setBackground(pill, ViewPolicy.ripple(s, fill, s.pixels(16)));
            pill.setContentDescription(description);
        }
        ViewPolicy.setVisibleIfChanged(pill, true);
    }
}
