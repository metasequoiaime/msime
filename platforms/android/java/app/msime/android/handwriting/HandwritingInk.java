package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/** Bounded, host-owned touch samples. Recognition remains an injected platform operation. */
public final class HandwritingInk {
    public static final int MAX_STROKES = 64;
    public static final int MAX_POINTS_PER_STROKE = 512;

    public record Point(float x, float y, long timeMillis) {
        public Point {
            if (!Float.isFinite(x) || !Float.isFinite(y) || x < 0 || y < 0
                    || timeMillis < 0) {
                throw new IllegalArgumentException("Handwriting point is invalid");
            }
        }
    }

    private final List<List<Point>> strokes = new ArrayList<>(MAX_STROKES);
    private boolean drawing;
    private long revision;

    public boolean begin(float x, float y, long timeMillis, float width, float height) {
        if (drawing || strokes.size() >= MAX_STROKES || !validBounds(width, height)) return false;
        List<Point> stroke = new ArrayList<>(MAX_POINTS_PER_STROKE);
        stroke.add(point(x, y, timeMillis, width, height));
        strokes.add(stroke);
        drawing = true;
        revision++;
        return true;
    }

    public boolean append(float x, float y, long timeMillis, float width, float height) {
        if (!drawing || strokes.isEmpty() || !validBounds(width, height)) return false;
        List<Point> stroke = strokes.get(strokes.size() - 1);
        if (stroke.size() >= MAX_POINTS_PER_STROKE) return false;
        Point next = point(x, y, timeMillis, width, height);
        Point previous = stroke.get(stroke.size() - 1);
        if (Math.hypot(previous.x() - next.x(), previous.y() - next.y()) < 1) return false;
        stroke.add(next);
        revision++;
        return true;
    }

    public boolean finish() {
        if (!drawing) return false;
        drawing = false;
        revision++;
        return true;
    }

    public boolean cancel() {
        if (!drawing || strokes.isEmpty()) return false;
        strokes.remove(strokes.size() - 1);
        drawing = false;
        revision++;
        return true;
    }

    public boolean undo() {
        if (drawing) return cancel();
        if (strokes.isEmpty()) return false;
        strokes.remove(strokes.size() - 1);
        revision++;
        return true;
    }

    public boolean clear() {
        if (strokes.isEmpty() && !drawing) return false;
        strokes.clear();
        drawing = false;
        revision++;
        return true;
    }

    public boolean hasInk() { return !strokes.isEmpty(); }
    public boolean isDrawing() { return drawing; }
    public long revision() { return revision; }

    /**
     * Current strokes for the package-owned canvas renderer.
     *
     * <p>The input view and this model are both confined to the main thread.  Drawing can therefore
     * walk the live lists without making the defensive deep copy required by {@link #snapshot()}.
     * Callers outside this package use {@code snapshot()} instead.
     */
    List<List<Point>> strokesForDrawing() { return strokes; }

    public List<List<Point>> snapshot() {
        // `Stream#toList` is API 34 and this host runs from API 28.
        List<List<Point>> copy = new java.util.ArrayList<>(strokes.size());
        for (List<Point> stroke : strokes) copy.add(List.copyOf(stroke));
        return List.copyOf(copy);
    }

    private static boolean validBounds(float width, float height) {
        return Float.isFinite(width) && Float.isFinite(height) && width > 0 && height > 0
            && width <= 4096 && height <= 4096;
    }

    private static Point point(float x, float y, long timeMillis, float width, float height) {
        if (!Float.isFinite(x) || !Float.isFinite(y)) {
            throw new IllegalArgumentException("Handwriting point is invalid");
        }
        return new Point(KeyboardGeometry.bounded(x, 0, width),
            KeyboardGeometry.bounded(y, 0, height),
            timeMillis);
    }
}
