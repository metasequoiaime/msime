package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.LinearGradient;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.Shader;
import android.util.AttributeSet;
import android.view.View;
import androidx.core.content.ContextCompat;
import app.msime.android.R;

/**
 * The daily line: a smooth curve over an accent wash that fades to nothing at the baseline, as the design draws it.
 *
 * The curve bends through horizontal midpoints, so every segment stays between its two days' values: a smooth line that invented a dip below zero or a peak nobody typed would be lying about the data.
 *
 * Drawn rather than charted: Material has no chart, and the alternative is a charting library whose
 * whole surface would be pulled in for one polyline. The axis is labelled from the data's own extent,
 * so an empty or flat series reads as flat instead of being scaled up into a false peak.
 */
public final class TrendChart extends View {
    private final Paint line = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint grid = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint wash = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path path = new Path();
    private final Path area = new Path();
    private final int accent;
    private int[] daily = new int[0];

    public TrendChart(Context context, AttributeSet attributes) {
        super(context, attributes);
        line.setStyle(Paint.Style.STROKE);
        line.setStrokeWidth(dp(2f));
        line.setStrokeCap(Paint.Cap.ROUND);
        line.setStrokeJoin(Paint.Join.ROUND);
        accent = ContextCompat.getColor(context, R.color.forest);
        line.setColor(accent);
        wash.setStyle(Paint.Style.FILL);
        grid.setStyle(Paint.Style.STROKE);
        grid.setStrokeWidth(dp(1f));
        grid.setColor(ContextCompat.getColor(context, R.color.hairline));
        label.setColor(ContextCompat.getColor(context, R.color.text_secondary));
        label.setTextSize(dp(10f));
    }

    public void setDaily(int[] daily) {
        this.daily = daily == null ? new int[0] : daily;
        invalidate();
    }

    private float dp(float value) { return value * getResources().getDisplayMetrics().density; }

    @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
        // Accent at 22% under the peak, fading out at the baseline.
        wash.setShader(new LinearGradient(0, dp(8f), 0, height - dp(18f),
            Color.argb(56, Color.red(accent), Color.green(accent), Color.blue(accent)),
            Color.argb(0, Color.red(accent), Color.green(accent), Color.blue(accent)),
            Shader.TileMode.CLAMP));
    }

    @Override protected void onDraw(Canvas canvas) {
        float right = getWidth() - dp(34f);
        float bottom = getHeight() - dp(18f);
        float top = dp(8f);
        int peak = 0;
        for (int value : daily) peak = Math.max(peak, value);

        for (int i = 0; i <= 3; i++) {
            float y = top + (bottom - top) * i / 3f;
            canvas.drawLine(0, y, right, y, grid);
            int mark = Math.round(peak * (3 - i) / 3f);
            canvas.drawText(String.valueOf(mark), right + dp(4f), y + dp(3f), label);
        }
        if (daily.length < 2 || peak == 0) return;

        path.reset();
        float lastX = 0f;
        float lastY = 0f;
        for (int i = 0; i < daily.length; i++) {
            float x = right * i / (float) (daily.length - 1);
            float y = bottom - (bottom - top) * daily[i] / (float) peak;
            if (i == 0) {
                path.moveTo(x, y);
            } else {
                float middle = (lastX + x) / 2f;
                path.cubicTo(middle, lastY, middle, y, x, y);
            }
            lastX = x;
            lastY = y;
        }
        area.set(path);
        area.lineTo(right, bottom);
        area.lineTo(0, bottom);
        area.close();
        canvas.drawPath(area, wash);
        canvas.drawPath(path, line);
    }
}
