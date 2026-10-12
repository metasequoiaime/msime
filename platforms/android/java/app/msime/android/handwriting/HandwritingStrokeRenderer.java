package app.msime.android;

import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import java.util.List;

/** Draws the same ink samples in the touch canvas and the stylus window. */
final class HandwritingStrokeRenderer {
    private HandwritingStrokeRenderer() {}

    static void configure(Paint stroke, float widthPixels) {
        stroke.setStyle(Paint.Style.STROKE);
        stroke.setStrokeCap(Paint.Cap.ROUND);
        stroke.setStrokeJoin(Paint.Join.ROUND);
        stroke.setStrokeWidth(widthPixels);
    }

    static void draw(Canvas canvas, Paint stroke, Path path, List<List<HandwritingInk.Point>> strokes) {
        for (List<HandwritingInk.Point> points : strokes) {
            if (points.isEmpty()) continue;
            if (points.size() == 1) {
                HandwritingInk.Point point = points.get(0);
                canvas.drawPoint(point.x(), point.y(), stroke);
                continue;
            }
            path.reset();
            path.moveTo(points.get(0).x(), points.get(0).y());
            for (int index = 1; index < points.size(); index++) {
                path.lineTo(points.get(index).x(), points.get(index).y());
            }
            canvas.drawPath(path, stroke);
        }
    }
}
