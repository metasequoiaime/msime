package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import app.msime.android.BoundsPolicy;
import app.msime.android.ColorPolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardSkin;
import app.msime.android.ViewPolicy;

/**
 * 一款皮肤的缩略：底色、几枚键帽，和那一枚强调键。
 *
 * <p>A catalogue of designs that shows only names and download counts asks the reader to pick a
 * colour scheme by its title. This is the smallest drawing that answers the question the list is
 * for -- what does it look like -- without downloading anything.
 */
public final class SkinSwatchView extends View {
    private static final int ROWS = 4;
    private static final int COLUMNS = 5;

    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    @Nullable private KeyboardSkin skin;

    public SkinSwatchView(Context context) { super(context); }

    public SkinSwatchView(Context context, AttributeSet attributes) { super(context, attributes); }

    /** Show one design, or nothing at all when the entry carries none. */
    public void setSkin(@Nullable KeyboardSkin value) {
        skin = value;
        ViewPolicy.setVisible(this, value != null);
        invalidate();
    }

    /** 当前显示的皮肤，没有时为 null。 */
    @Nullable public KeyboardSkin skin() { return skin; }

    /**
     * 按设计的皮肤卡缩略画：皮肤底色上四行小键帽，前三行是字母键，第三行两端和底行两侧是功能键底色，底行中间是空格，右下角是回车色。
     */
    @Override protected void onDraw(Canvas canvas) {
        KeyboardSkin value = skin;
        if (value == null || getWidth() <= 0 || getHeight() <= 0) return;
        float radius = KeyboardGeometry.floatPixels(getContext(), 10);
        paint.setColor(ColorPolicy.parse(value.background(), Color.LTGRAY));
        box.set(0, 0, getWidth(), getHeight());
        canvas.drawRoundRect(box, radius, radius, paint);

        float pad = KeyboardGeometry.floatPixels(getContext(), 5);
        float gap = KeyboardGeometry.floatPixels(getContext(), 2.5f);
        float cellWidth = (getWidth() - pad * 2 - gap * (COLUMNS - 1)) / COLUMNS;
        float cellHeight = (getHeight() - pad * 2 - gap * (ROWS - 1)) / ROWS;
        if (cellWidth <= 0 || cellHeight <= 0) return;
        float capRadius = BoundsPolicy.atMost(
            KeyboardGeometry.floatPixels(getContext(), (float) value.cornerRadius()) / 2f,
            cellHeight / 2.5f);
        int cap = ColorPolicy.parse(value.keyBackground(), Color.WHITE);
        int function = ColorPolicy.parse(value.functionBackground(), cap);
        int action = ColorPolicy.parse(value.returnBackground(),
            ColorPolicy.parse(value.actionBackground(), Color.DKGRAY));
        for (int row = 0; row < ROWS; row++) {
            float top = pad + row * (cellHeight + gap);
            if (row == ROWS - 1) {
                // 底行：功能键、空格（占中间三格）、回车。
                float left = pad;
                drawCap(canvas, left, top, cellWidth, cellHeight, capRadius, function);
                left += cellWidth + gap;
                float space = cellWidth * (COLUMNS - 2) + gap * (COLUMNS - 3);
                drawCap(canvas, left, top, space, cellHeight, capRadius, cap);
                left += space + gap;
                drawCap(canvas, left, top, cellWidth, cellHeight, capRadius, action);
                continue;
            }
            for (int column = 0; column < COLUMNS; column++) {
                boolean edge = row == ROWS - 2 && (column == 0 || column == COLUMNS - 1);
                float left = pad + column * (cellWidth + gap);
                drawCap(canvas, left, top, cellWidth, cellHeight, capRadius, edge ? function : cap);
            }
        }
    }

    private void drawCap(Canvas canvas, float left, float top, float width, float height,
            float radius, int color) {
        paint.setColor(color);
        box.set(left, top, left + width, top + height);
        canvas.drawRoundRect(box, radius, radius, paint);
    }
}
