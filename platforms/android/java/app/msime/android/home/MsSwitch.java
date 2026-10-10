package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.ColorPolicy;
import android.content.Context;
import android.graphics.Color;
import android.util.AttributeSet;
import androidx.annotation.Nullable;
import com.google.android.material.materialswitch.MaterialSwitch;

/**
 * 设计里的 M3 开关：52×32 的轨道，打开时滑块 24dp、关闭时 16dp，关闭态有 2dp 描边。
 *
 * <p>尺寸和滑块大小变化就是 `MaterialSwitch` 自己的；这里只把颜色接到主题属性上：打开时轨道是 accent、滑块是 `colorOnPrimary`（深色模式下由每个季节的深色 onAccent 给出，不用原型漏出来的 `#003920`），关闭时轨道是 `colorSurfaceContainerHighest`、滑块和描边是 `colorOutline`。
 */
public final class MsSwitch extends MaterialSwitch {
    public MsSwitch(Context context) {
        this(context, null);
    }

    public MsSwitch(Context context, @Nullable AttributeSet attrs) {
        super(context, attrs);
        int[][] states = {{android.R.attr.state_checked}, {}};
        int accent = ThemeColorPolicy.accent(context);
        int onAccent = ThemeColorPolicy.onAccent(context);
        int off = ThemeColorPolicy.color(context, com.google.android.material.R.attr.colorSurfaceContainerHighest);
        int outline = ThemeColorPolicy.outline(context);
        // 禁用态整体变淡由所在的行负责（Ui.setEnabledLook），这里颜色只按开与关区分，禁用的打开态仍然看得出是打开的。
        setTrackTintList(ColorPolicy.stateList(states, new int[] {accent, off}));
        setThumbTintList(ColorPolicy.stateList(states, new int[] {onAccent, outline}));
        setTrackDecorationTintList(ColorPolicy.stateList(states,
            new int[] {Color.TRANSPARENT, outline}));
        setThumbIconDrawable(null);
        setText(null);
    }
}
