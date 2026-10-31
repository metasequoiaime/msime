package app.msime.android;

import android.graphics.Canvas;
import android.graphics.ColorFilter;
import android.graphics.Paint;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.graphics.drawable.Drawable;
import android.widget.TextView;

/**
 * 读音行的「编辑拼写」入口：拼写已经写在输入框里、读音行不再画拼写时，读音行只画一支笔，点它打开放大编辑条。原来这里放的是「编辑拼写」四个字，一打字就出现在候选栏正上方，读起来像状态却不带任何信息。
 *
 * <p>颜色每次绘制时取读音 TextView 当前的文字色，跟着皮肤和 {@code ImeToolbar.styleTopRow} 走，不另存一份。读屏照旧念读音行的描述「编辑拼写」。
 */
public final class SpellingEditIcon extends Drawable {
    private final TextView owner;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);

    public SpellingEditIcon(TextView owner) {
        this.owner = owner;
    }

    @Override public void draw(Canvas canvas) {
        Rect bounds = getBounds();
        KeyboardIconPaths.draw(canvas, paint, KeyboardIconPaths.Icon.SPELLING_EDIT, bounds.left, bounds.top,
            Math.min(bounds.width(), bounds.height()), owner.getCurrentTextColor());
    }

    @Override public void setAlpha(int alpha) {
        // 颜色整个取自文字色（含透明度），不单独调透明度。
    }

    @Override public void setColorFilter(ColorFilter filter) {
        paint.setColorFilter(filter);
    }

    @SuppressWarnings("deprecation")
    @Override public int getOpacity() {
        return PixelFormat.TRANSLUCENT;
    }
}
