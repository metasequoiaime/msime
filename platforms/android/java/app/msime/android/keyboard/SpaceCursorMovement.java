package app.msime.android;

/** Bounded space-drag distance accumulator tied to one editor connection. */
public final class SpaceCursorMovement {
    private static final float MAXIMUM_JUMP = 4096;
    private float previous;
    private float remainder;
    private Object document;

    public boolean isActive() { return document != null; }

    public void begin(float position, Object document) {
        if (!Float.isFinite(position) || document == null) {
            cancel();
            return;
        }
        this.document = document;
        previous = position;
        remainder = 0;
    }

    public int advance(float position, Object document, float pixelsPerStep) {
        if (!isActive() || this.document != document || !Float.isFinite(position)
                || !Float.isFinite(pixelsPerStep) || pixelsPerStep < 1) {
            cancel();
            return 0;
        }
        float change = position - previous;
        if (Math.abs(change) > MAXIMUM_JUMP) {
            cancel();
            return 0;
        }
        remainder += change;
        previous = position;
        int steps = (int) (remainder / pixelsPerStep);
        remainder -= steps * pixelsPerStep;
        return steps;
    }

    public void cancel() {
        document = null;
        previous = 0;
        remainder = 0;
    }
}
