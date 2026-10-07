package app.msime.android.home;

import app.msime.android.KeyboardGeometry;
import android.content.Context;
import android.graphics.drawable.ClipDrawable;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.LayerDrawable;
import android.util.AttributeSet;
import android.view.Gravity;
import android.widget.SeekBar;
import androidx.annotation.Nullable;
import app.msime.android.ViewPolicy;
import java.util.function.IntConsumer;

/**
 * 设计里的 M3 滑块：6dp 的轨道（已选部分 accent、其余 accentSoft），4×28dp、圆角 2 的条形滑块。
 *
 * <p>建在 `SeekBar` 上而不是 Material 的 `Slider`：条形滑块是 Material 1.13 之后才有的样式，而 Tauri 合包编译同一份源码时只有 Material 1.12（计划 G13）。`SeekBar` 自带读屏的「调整」操作和方向键，这里只换它的两块 drawable，并把取值换成带步长的 [min, max] 区间。
 */
public final class MsSlider extends SeekBar {
    private int min;
    private int step = 1;
    @Nullable private IntConsumer onChange;
    @Nullable private IntConsumer onCommit;
    private boolean tracking;

    public MsSlider(Context context) {
        this(context, null);
    }

    public MsSlider(Context context, @Nullable AttributeSet attrs) {
        super(context, attrs);
        int track = Ui.dp(context, Ui.SLIDER_TRACK);
        float radius = track / 2f;

        GradientDrawable rest = Ui.rounded(Ui.accentSoft(context), radius);
        GradientDrawable done = Ui.rounded(Ui.accent(context), radius);
        Drawable progress = new ClipDrawable(done, Gravity.START, ClipDrawable.HORIZONTAL);
        LayerDrawable layers = new LayerDrawable(new Drawable[] {rest, progress});
        layers.setId(0, android.R.id.background);
        layers.setId(1, android.R.id.progress);
        // 轨道只有 6dp 粗，竖直居中在 30dp 的点按区里；不能用 setMaxHeight，它要 API 29，而本应用最低支持 28。
        for (int i = 0; i < 2; i++) {
            layers.setLayerHeight(i, track);
            layers.setLayerGravity(i, Gravity.CENTER_VERTICAL | Gravity.FILL_HORIZONTAL);
        }
        setProgressDrawable(layers);

        GradientDrawable thumb = Ui.rounded(Ui.accent(context), Ui.dp(context, 2));
        thumb.setSize(Ui.dp(context, Ui.SLIDER_THUMB_WIDTH), Ui.dp(context, Ui.SLIDER_THUMB_HEIGHT));
        setThumb(thumb);
        setThumbOffset(Ui.dp(context, Ui.SLIDER_THUMB_WIDTH) / 2);
        setSplitTrack(false);
        ViewPolicy.clearBackground(this);
        // 左右留出半个滑块，滑块在两端时不会被裁掉。
        int inset = Ui.dp(context, Ui.SLIDER_THUMB_WIDTH);
        Ui.setHorizontalPaddingPx(this, inset);
        Ui.setMinimumHeightDp(this, context, Ui.SLIDER_TOUCH_HEIGHT);

        setOnSeekBarChangeListener(new OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar bar, int progress, boolean fromUser) {
                if (onChange != null) onChange.accept(value());
                // 方向键和读屏的「调整」不经过按下与松手，每一步本身就是一次完成的修改。
                if (fromUser && !tracking && onCommit != null) onCommit.accept(value());
            }

            @Override public void onStartTrackingTouch(SeekBar bar) { tracking = true; }

            @Override public void onStopTrackingTouch(SeekBar bar) {
                tracking = false;
                if (onCommit != null) onCommit.accept(value());
            }
        });
    }

    /** 设置取值区间与步长；当前值会被夹进新区间并对齐到步长。 */
    public void setRange(int min, int max, int step) {
        if (max <= min || step <= 0) throw new IllegalArgumentException("bad slider range");
        int current = value();
        this.min = min;
        this.step = step;
        setMax((max - min) / step);
        setValue(current);
    }

    /** 当前值（已换算到区间里）。 */
    public int value() {
        return min + getProgress() * step;
    }

    /** 设置当前值，不在区间里时夹到端点，不在步长上时取最近的一格。 */
    public void setValue(int value) {
        int slot = Math.round((value - min) / (float) step);
        setProgress(KeyboardGeometry.bounded(slot, 0, getMax()));
    }

    /** 拖动过程中每一格都回调，用来实时刷新数值标签。 */
    public void setOnValueChange(@Nullable IntConsumer listener) {
        onChange = listener;
    }

    /** 一次修改完成时回调，用来保存：拖动时在松手那一刻，方向键和读屏调整时每一步一次。 */
    public void setOnValueCommit(@Nullable IntConsumer listener) {
        onCommit = listener;
    }
}
