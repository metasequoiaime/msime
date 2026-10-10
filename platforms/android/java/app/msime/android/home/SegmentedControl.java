package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.util.AttributeSet;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.LinearLayout;
import android.widget.RadioButton;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.DrawablePolicy;
import app.msime.android.ViewPolicy;
import java.util.ArrayList;
import java.util.List;
import java.util.function.IntConsumer;

/**
 * 设计里的 Android 分段控件：1dp 描边的胶囊容器，内边距 2、段间距 2，选中段填 accentSoft、文字 accent，未选中段文字为次要文字色，13sp。
 *
 * <p>自己画而不用 `MaterialButtonToggleGroup`：后者把各段画成相连的描边按钮，和设计「容器描边、选中段是里面一枚胶囊」的样子不同；Material 1.13 的 `MaterialButtonGroup` 又超出了 Tauri 合包的 Material 1.12（计划 G13）。原型漏出的绿色选中底（`#CFE9D6` / `#2A4F37`）在这里一律是 accentSoft。读屏把每一段读成单选按钮并报告选中状态。
 */
public final class SegmentedControl extends LinearLayout {
    private final ArrayList<TextView> segments = new ArrayList<>();
    private int selected = -1;
    private boolean fill;
    @Nullable private IntConsumer listener;

    public SegmentedControl(Context context) {
        this(context, null);
    }

    public SegmentedControl(Context context, @Nullable AttributeSet attrs) {
        super(context, attrs);
        setOrientation(HORIZONTAL);
        ViewPolicy.setCenteredVertically(this);
        int pad = Ui.dp(context, 2);
        Ui.setSymmetricPaddingPx(this, pad);
        GradientDrawable frame = DrawablePolicy.outlined(Color.TRANSPARENT, 9999f, Ui.dp(context, 1),
            ThemeColorPolicy.outline(context));
        setBackground(frame);
    }

    /** 换一组选项；`selected` 为 -1 表示没有选中项。不会回调监听器。 */
    public void setOptions(List<? extends CharSequence> labels, int selected) {
        removeAllViews();
        segments.clear();
        segments.ensureCapacity(labels.size());
        Context context = getContext();
        for (int i = 0; i < labels.size(); i++) {
            int index = i;
            TextView segment = Ui.centeredSingleLineLabel(context, labels.get(i), Ui.TEXT_SEGMENT, 400,
                ThemeColorPolicy.subText(context));
            Ui.setTextMinHeightDp(segment, context, 28);
            // 平分宽度时段宽已经定死、文字居中，左右内边距只会挤掉文字：AI 设计皮肤的「按键音效」五段在 360dp 宽的屏上每段约 63dp，左右各 12dp 后只剩 39dp，系统字号稍大就把「打字机」截成「打字札」（#6070）。按文字宽度排开时内边距决定段宽，保持 12dp。
            Ui.setSymmetricPaddingDp(segment, context, fill ? 4 : 12, 4);
            ViewPolicy.setInteractive(segment, true);
            ViewPolicy.bindClick(segment, () -> select(index, true));
            segment.setAccessibilityDelegate(new AccessibilityDelegate() {
                @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfo info) {
                    super.onInitializeAccessibilityNodeInfo(host, info);
                    info.setClassName(RadioButton.class.getName());
                    info.setCheckable(true);
                    info.setChecked(index == SegmentedControl.this.selected);
                }
            });
            LayoutParams params = fill
                ? new LayoutParams(0, LayoutParams.MATCH_PARENT, 1f)
                : new LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.MATCH_PARENT);
            if (i > 0) params.setMarginStart(Ui.dp(context, 2));
            addView(segment, params);
            segments.add(segment);
        }
        this.selected = -1;
        select(selected, false);
    }

    /** 为真时各段平分整个宽度（社区页那种通栏分段），否则按文字宽度排开。要在 setOptions 之前调用。 */
    public void setFillWidth(boolean fill) {
        this.fill = fill;
    }

    /** 用户点了某一段之后回调，参数是新选中的下标。 */
    public void setOnSelect(@Nullable IntConsumer listener) {
        this.listener = listener;
    }

    public int selected() { return selected; }

    /** 从代码里改选中项，不回调监听器。 */
    public void setSelection(int index) {
        select(index, false);
    }

    private void select(int index, boolean fromUser) {
        if (index < -1 || index >= segments.size()) return;
        boolean changed = index != selected;
        selected = index;
        Context context = getContext();
        int accent = ThemeColorPolicy.accent(context);
        int sub = ThemeColorPolicy.subText(context);
        for (int i = 0; i < segments.size(); i++) {
            TextView segment = segments.get(i);
            boolean on = i == index;
            Ui.style(segment, Ui.TEXT_SEGMENT, on ? 500 : 400, on ? accent : sub);
            ViewPolicy.setBackground(segment, Ui.pillRipple(context, on ? ThemeColorPolicy.accentSoft(context) : Color.TRANSPARENT));
            ViewPolicy.setSelected(segment, on);
        }
        if (fromUser && changed && listener != null) listener.accept(index);
    }
}
