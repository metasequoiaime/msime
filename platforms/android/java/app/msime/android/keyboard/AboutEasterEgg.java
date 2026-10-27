package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.text.Layout;
import android.text.StaticLayout;
import android.text.TextPaint;
import android.util.TypedValue;
import android.view.View;

import java.time.LocalDate;

/** 关于页长按彩蛋：一张完全本地绘制的小场景，不依赖网络或新增图片资源。 */
public final class AboutEasterEgg {
    private AboutEasterEgg() {}

    /** 键盘菜单直接承载场景，不从输入法服务弹出需要 Activity 令牌的对话框。 */
    public static View scene(Context context, int background) {
        return new Scene(context, AboutEasterEggPolicy.message(LocalDate.now()), background);
    }

    private static final class Scene extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final TextPaint textPaint = new TextPaint(Paint.ANTI_ALIAS_FLAG);
        private final String message;
        private final float density;
        private final int background;
        private StaticLayout text;

        Scene(Context context, String message, int background) {
            super(context);
            this.message = message;
            this.background = background;
            density = getResources().getDisplayMetrics().density;
            textPaint.setColor(Color.WHITE);
            textPaint.setTextSize(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, 14,
                getResources().getDisplayMetrics()));
            textPaint.setTypeface(android.graphics.Typeface.DEFAULT_BOLD);
            setContentDescription(message + " 水杉妹坐在树下刷手机");
        }

        private float dp(float value) { return value * density; }

        private StaticLayout text(float width) {
            int available = Math.max(1, Math.round(width - dp(48)));
            if (text == null || text.getWidth() != available) {
                text = StaticLayout.Builder.obtain(message, 0, message.length(), textPaint, available)
                    .setAlignment(Layout.Alignment.ALIGN_CENTER)
                    .setIncludePad(false)
                    .build();
            }
            return text;
        }

        @Override protected void onMeasure(int widthSpec, int heightSpec) {
            int width = resolveSize(Math.round(dp(320)), widthSpec);
            float scale = Math.min(1.35f, width / dp(320));
            int height = Math.round(dp(8) + text(width).getHeight() + dp(24 + 12 + 8) + dp(220) * scale);
            setMeasuredDimension(width, resolveSize(height, heightSpec));
        }

        @Override protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            float width = getWidth();
            float height = getHeight();
            paint.setStyle(Paint.Style.FILL);
            // Paint 跨帧复用，上一帧人物的粗线不能改变下一帧草叶和手臂的笔宽。
            paint.setStrokeWidth(dp(1));
            paint.setColor(background);
            canvas.drawRect(0, 0, width, height, paint);

            float bubbleBottom = dp(8 + 24) + text(width).getHeight();
            float artTop = bubbleBottom + dp(12);
            float scale = Math.min(1.35f, Math.min(width / dp(320), Math.max(0, height - artTop) / dp(220)));
            float grassTop = artTop + dp(190) * scale;
            paint.setColor(Color.rgb(190, 224, 166));
            canvas.drawRect(0, grassTop, width, height, paint);
            paint.setColor(Color.rgb(131, 190, 112));
            for (int i = 0; i < 12; i++) {
                float x = dp(8) + i * width / 12f;
                canvas.drawLine(x, grassTop + dp(8), x - dp(3), grassTop - dp(3), paint);
                canvas.drawLine(x + dp(4), grassTop + dp(8), x + dp(8), grassTop - dp(1), paint);
            }

            drawBubble(canvas, width, bubbleBottom);
            canvas.save();
            canvas.translate((width - dp(320) * scale) / 2, artTop);
            canvas.scale(scale, scale);
            drawTree(canvas, dp(320), dp(190));
            drawPerson(canvas, dp(320), dp(190));
            canvas.restore();
        }

        private void drawBubble(Canvas canvas, float width, float bottom) {
            float left = dp(12), top = dp(8), right = width - dp(12);
            paint.setColor(Color.rgb(234, 93, 95));
            canvas.drawRoundRect(new RectF(left, top, right, bottom), dp(16), dp(16), paint);
            Path tail = new Path();
            tail.moveTo(right - dp(42), bottom);
            tail.lineTo(right - dp(22), bottom);
            tail.lineTo(right - dp(34), bottom + dp(15));
            tail.close();
            canvas.drawPath(tail, paint);

            StaticLayout text = text(width);
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
            float x = width * .40f;
            float headY = grassTop - dp(58);
            paint.setColor(Color.rgb(255, 211, 179));
            canvas.drawCircle(x, headY, dp(17), paint);
            paint.setColor(Color.rgb(61, 151, 81));
            canvas.drawCircle(x - dp(2), headY - dp(8), dp(19), paint);
            canvas.drawOval(new RectF(x - dp(26), headY - dp(8), x - dp(10), headY + dp(24)), paint);
            canvas.drawOval(new RectF(x + dp(10), headY - dp(7), x + dp(25), headY + dp(28)), paint);
            paint.setColor(Color.rgb(255, 211, 179));
            canvas.drawOval(new RectF(x - dp(12), headY - dp(1), x + dp(13), headY + dp(15)), paint);
            paint.setColor(Color.rgb(112, 78, 38));
            paint.setStrokeWidth(dp(2));
            canvas.drawLine(x - dp(7), headY + dp(5), x - dp(2), headY + dp(6), paint);
            canvas.drawLine(x + dp(4), headY + dp(6), x + dp(9), headY + dp(5), paint);
            canvas.drawLine(x, headY + dp(11), x + dp(4), headY + dp(11), paint);

            paint.setColor(Color.rgb(73, 163, 93));
            Path dress = new Path();
            dress.moveTo(x - dp(18), headY + dp(15));
            dress.lineTo(x + dp(18), headY + dp(15));
            dress.lineTo(x + dp(30), grassTop - dp(8));
            dress.lineTo(x - dp(27), grassTop - dp(8));
            dress.close();
            canvas.drawPath(dress, paint);

            paint.setColor(Color.rgb(255, 211, 179));
            paint.setStrokeWidth(dp(5));
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
