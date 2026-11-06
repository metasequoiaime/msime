package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.drawable.Drawable;

/** 中央单元格由网格画公共底面，只在按下或选择时填充自身区域。 */
public class TextEditPanelButton extends KeyboardPressButton {
    private boolean seamless;
    private int selectedColor;
    private final Paint surface = new Paint();

    public TextEditPanelButton(Context context) { super(context); }

    public void setSeamless(int accent) {
        seamless = true;
        selectedColor = accent;
        super.setBackground(null);
        int inset = DimensionPolicy.pixels(getContext(), 6);
        ViewPolicy.setPadding(this, inset, 0, inset, 0);
        setAnimatesPressFeedback(false);
        invalidate();
    }

    @Override public void setBackground(Drawable drawable) {
        super.setBackground(seamless ? null : drawable);
    }

    protected final void drawPanelState(Canvas canvas) {
        if (!seamless || (!isSelected() && !isPressed())) return;
        surface.setColor(isSelected() ? selectedColor
            : ColorPolicy.withAlpha(getCurrentTextColor(), .10f));
        canvas.drawRect(0, 0, getWidth(), getHeight(), surface);
    }

    @Override protected void onDraw(Canvas canvas) {
        drawPanelState(canvas);
        super.onDraw(canvas);
    }
}
