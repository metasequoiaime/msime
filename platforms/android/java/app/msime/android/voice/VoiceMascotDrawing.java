package app.msime.android;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;

/** 外部听写与键盘工具栏共用水杉娘耳部和音量信号的绘制，素材只保留现有贴纸一份。 */
public final class VoiceMascotDrawing {
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
    private final RectF character = new RectF();
    private final RectF arc = new RectF();
    private final Path ear = new Path();

    public void draw(Canvas canvas, Bitmap mascot, float width, float height, float level) {
        float scale = Math.min(width / 320f, height / 250f);
        if (scale <= 0) return;
        int saved = canvas.save();
        canvas.translate((width - 320 * scale) / 2, (height - 250 * scale) / 2);
        canvas.scale(scale, scale);
        character.set(20, 12, 248, 240);
        paint.setAlpha(255);
        paint.setStyle(Paint.Style.FILL);
        if (mascot != null) canvas.drawBitmap(mascot, null, character, paint);

        // 在原素材右耳的位置画放大的卡通耳部，保留人物主体和同一套贴纸来源。
        ear.reset();
        ear.moveTo(187, 109);
        ear.cubicTo(189, 76, 235, 77, 234, 109);
        ear.cubicTo(235, 127, 215, 132, 213, 145);
        ear.cubicTo(207, 158, 186, 145, 190, 130);
        ear.close();
        paint.setColor(Color.rgb(255, 220, 196));
        canvas.drawPath(ear, paint);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(2.5f);
        paint.setStrokeCap(Paint.Cap.ROUND);
        paint.setColor(Color.rgb(157, 92, 77));
        canvas.drawPath(ear, paint);
        ear.reset();
        ear.moveTo(199, 111);
        ear.cubicTo(198, 90, 224, 91, 222, 109);
        ear.cubicTo(222, 118, 205, 117, 206, 131);
        canvas.drawPath(ear, paint);

        if (level > 0) {
            paint.setColor(Color.rgb(211, 47, 47));
            paint.setAlpha(90 + Math.round(165 * level));
            paint.setStrokeWidth(3 + 2 * level);
            for (int index = 0; index < 3; index++) {
                if (level < index * .3f) continue;
                float radius = 33 + index * 15 + level * 12;
                arc.set(218 - radius, 111 - radius, 218 + radius, 111 + radius);
                canvas.drawArc(arc, -48, 96, false, paint);
            }
        }
        canvas.restoreToCount(saved);
    }
}
