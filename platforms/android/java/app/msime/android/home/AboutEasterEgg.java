package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.text.Layout;
import android.text.StaticLayout;
import android.text.TextPaint;
import android.view.View;
import androidx.annotation.NonNull;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import app.msime.android.AboutEasterEggPolicy;
import app.msime.android.DimensionPolicy;
import app.msime.android.ThemeColorPolicy;

import java.time.LocalDate;

/** 关于页长按彩蛋：一张完全本地绘制的小场景，不依赖网络或新增图片资源。 */
final class AboutEasterEgg {
    private AboutEasterEgg() {}

    static void show(Context context) {
        Scene scene = new Scene(context, AboutEasterEggPolicy.message(LocalDate.now()));
        int horizontal = DimensionPolicy.pixels(context, 12);
        new MaterialAlertDialogBuilder(context)
            .setTitle("水杉的小秘密")
            .setView(scene, horizontal, 0, horizontal, 0)
            .setPositiveButton("好", null)
            .show();
    }

    private static final class Scene extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final TextPaint textPaint = new TextPaint(Paint.ANTI_ALIAS_FLAG);
        private final String message;
        private final float density;

        Scene(Context context, String message) {
            super(context);
            this.message = message;
            density = getResources().getDisplayMetrics().density;
            setMinimumHeight(DimensionPolicy.pixels(context, 250));
            setContentDescription(message + " 水杉妹坐在树下刷手机");
        }

        private float dp(float value) { return value * density; }

        @Override protected void onDraw(@NonNull Canvas canvas) {
            super.onDraw(canvas);
            float width = getWidth();
            float height = getHeight();
            paint.setStyle(Paint.Style.FILL);
            paint.setColor(ThemeColorPolicy.page(getContext()));
            canvas.drawRect(0, 0, width, height, paint);

            float grassTop = height * .70f;
            paint.setColor(Color.rgb(190, 224, 166));
            canvas.drawRect(0, grassTop, width, height, paint);
            paint.setColor(Color.rgb(131, 190, 112));
            for (int i = 0; i < 12; i++) {
                float x = dp(8) + i * width / 12f;
                canvas.drawLine(x, grassTop + dp(8), x - dp(3), grassTop - dp(3), paint);
                canvas.drawLine(x + dp(4), grassTop + dp(8), x + dp(8), grassTop - dp(1), paint);
            }

            drawBubble(canvas, width, height);
            drawTree(canvas, width, grassTop);
            drawPerson(canvas, width, grassTop);
        }

        private void drawBubble(Canvas canvas, float width, float height) {
            float left = dp(12), top = dp(8), right = width - dp(12), bottom = height * .30f;
            paint.setColor(Color.rgb(234, 93, 95));
            canvas.drawRoundRect(new RectF(left, top, right, bottom), dp(16), dp(16), paint);
            Path tail = new Path();
            tail.moveTo(right - dp(42), bottom);
            tail.lineTo(right - dp(22), bottom);
            tail.lineTo(right - dp(34), bottom + dp(15));
            tail.close();
            canvas.drawPath(tail, paint);

            textPaint.setColor(Color.WHITE);
            textPaint.setTextSize(dp(14));
            textPaint.setTypeface(android.graphics.Typeface.DEFAULT_BOLD);
            StaticLayout text = StaticLayout.Builder.obtain(message, 0, message.length(), textPaint,
                    (int) (right - left - dp(24)))
                .setAlignment(Layout.Alignment.ALIGN_CENTER)
                .setIncludePad(false)
                .build();
            canvas.save();
            canvas.translate(left + dp(12), top + (bottom - top - text.getHeight()) / 2f);
            text.draw(canvas);
            canvas.restore();
        }

        private void drawTree(Canvas canvas, float width, float grassTop) {
            float x = width * .19f;
            paint.setColor(Color.rgb(123, 78, 50));
            canvas.drawRoundRect(new RectF(x - dp(9), grassTop - dp(118), x + dp(11), grassTop + dp(6)), dp(8), dp(8), paint);
            paint.setColor(Color.rgb(52, 137, 75));
            canvas.drawCircle(x, grassTop - dp(126), dp(38), paint);
            paint.setColor(Color.rgb(73, 165, 82));
            canvas.drawCircle(x - dp(22), grassTop - dp(104), dp(30), paint);
            canvas.drawCircle(x + dp(24), grassTop - dp(102), dp(31), paint);
            paint.setColor(Color.rgb(101, 186, 88));
            canvas.drawCircle(x, grassTop - dp(155), dp(25), paint);
        }

        private void drawPerson(Canvas canvas, float width, float grassTop) {
            float x = width * .56f;
            float headY = grassTop - dp(58);
            paint.setColor(Color.rgb(255, 211, 179));
            canvas.drawCircle(x, headY, dp(17), paint);
            paint.setColor(Color.rgb(61, 151, 81));
            canvas.drawCircle(x - dp(2), headY - dp(8), dp(19), paint);
            canvas.drawOval(new RectF(x - dp(26), headY - dp(8), x - dp(10), headY + dp(24)), paint);
            canvas.drawOval(new RectF(x + dp(10), headY - dp(7), x + dp(25), headY + dp(28)), paint);

            paint.setColor(Color.rgb(73, 163, 93));
            Path dress = new Path();
            dress.moveTo(x - dp(18), headY + dp(15));
            dress.lineTo(x + dp(18), headY + dp(15));
            dress.lineTo(x + dp(30), grassTop - dp(8));
            dress.lineTo(x - dp(27), grassTop - dp(8));
            dress.close();
            canvas.drawPath(dress, paint);

            paint.setColor(Color.rgb(255, 211, 179));
            canvas.drawLine(x - dp(12), headY + dp(25), x - dp(32), grassTop - dp(22), paint);
            canvas.drawLine(x + dp(12), headY + dp(25), x + dp(28), grassTop - dp(25), paint);
            paint.setStrokeWidth(dp(7));
            canvas.drawLine(x - dp(20), grassTop - dp(8), x - dp(36), grassTop + dp(3), paint);
            canvas.drawLine(x + dp(12), grassTop - dp(8), x + dp(29), grassTop + dp(3), paint);

            paint.setColor(Color.rgb(48, 74, 90));
            canvas.drawRoundRect(new RectF(x + dp(19), headY + dp(28), x + dp(42), headY + dp(45)), dp(3), dp(3), paint);
            paint.setColor(Color.rgb(153, 219, 196));
            canvas.drawRect(new RectF(x + dp(22), headY + dp(31), x + dp(39), headY + dp(41)), paint);
        }
    }
}
