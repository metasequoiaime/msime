package app.msime.android;

import app.msime.android.LayoutPolicy;

import android.graphics.Color;
import android.view.MotionEvent;
import android.widget.FrameLayout;
import app.msime.android.core.InputViewValuePolicy;
import java.util.Arrays;

/**
 * 26 键上的滑行输入（{@link GlideTypingPolicy}）。按键区把每一个触摸事件先交给这里：一次落在字母键上的按下滑到另一个字母键时，这里接管这一次触摸，按下的键收到 CANCEL、什么也不输入，键盘上画出手指的轨迹；抬手时把 26 个字母键的位置和整条轨迹交给引擎，由引擎写进组字。没有构成滑行的按下原样交给键，点按、长按和「滑动输入符号」都不受影响。
 */
final class ImeGlideTyping implements KeyboardKeyArea.GlideTracker {
    private static final int LETTERS = 26;
    private static final float TRAIL_WIDTH_DP = 4f;

    private final MSIMEInputService s;
    /** 按下时量好的各键在按键区里的矩形。滑行只认每个字母的第一个键：分离式键盘右半边重复的 G、V（#6022）在滑行里和空隙一样不算字母键，交给 Engine 的键位每个字母只有一个中心点。 */
    private final LetterKeyMeasure keys = new LetterKeyMeasure();
    private final int[] areaLocation = new int[2];
    private final int[] trailLocation = new int[2];
    /** 这一次按下可能变成滑行：落在字母键上、满足开始条件，还没有第二根手指。 */
    private boolean tracking;
    /** 已经构成滑行，正在收集轨迹。 */
    private boolean gliding;
    /** 从构成滑行到这一次触摸结束（最后一根手指抬起或取消），事件都不再交给键。 */
    private boolean owned;
    private int pointerId;
    private char downLetter;
    private float downX;
    private long downTime;
    private float[] xs = new float[128];
    private float[] ys = new float[128];
    private long[] times = new long[128];
    private int count;
    private GlideTrailView trail;
    private float trailOffsetX;
    private float trailOffsetY;

    ImeGlideTyping(MSIMEInputService s) {
        this.s = s;
    }

    @Override public boolean track(KeyboardKeyArea area, MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                reset();
                begin(area, event);
                return false;
            }
            case MotionEvent.ACTION_POINTER_DOWN -> {
                // 第二根手指先落下时，这一次按下就是多指连按，不再看成滑行。
                if (!gliding) tracking = false;
                return owned;
            }
            case MotionEvent.ACTION_MOVE -> {
                if (tracking) follow(area, event);
                return owned;
            }
            case MotionEvent.ACTION_POINTER_UP -> {
                if (event.getPointerId(event.getActionIndex()) == pointerId) {
                    if (gliding) finish(event, event.getActionIndex());
                    tracking = false;
                }
                return owned;
            }
            case MotionEvent.ACTION_UP -> {
                boolean wasOwned = owned;
                if (gliding && event.getPointerId(0) == pointerId) finish(event, 0);
                reset();
                return wasOwned;
            }
            case MotionEvent.ACTION_CANCEL -> {
                boolean wasOwned = owned;
                reset();
                return wasOwned;
            }
            default -> {
                return owned;
            }
        }
    }

    private void begin(KeyboardKeyArea area, MotionEvent event) {
        boolean enabled = s.localSettings.bool(AndroidLocalSettings.GLIDE_TYPING);
        if (!enabled || s.view == null) return;
        boolean standardLetters = s.keyboardLayer == KeyboardLayout.Layer.LETTERS
            && s.displayedTouchLayout(s.view) == KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        if (!standardLetters) return;
        int measured = keys.measure(area, s.shuangpinKeyButtons, s.shuangpinKeyInputs);
        String localMode = JsonPolicy.strictStringOrEmpty(s.view.opt("local_mode"));
        if (localMode.isEmpty()) localMode = "none";
        if (!GlideTypingPolicy.armed(true, true, InputViewValuePolicy.scheme(s.view, -1),
                s.dedicatedEnglish, localMode, measured)) return;
        float x = event.getX(0);
        float y = event.getY(0);
        char letter = letterAt(x, y);
        if (letter == 0) return;
        tracking = true;
        pointerId = event.getPointerId(0);
        downLetter = letter;
        downX = x;
        downTime = event.getDownTime();
        add(x, y, event.getEventTime());
    }

    /** (x, y) 所在的字母键，不在任何字母键上（键距、功能键）时为 0。 */
    private char letterAt(float x, float y) {
        for (int letter = 0; letter < LETTERS; letter++) {
            int slot = letter * 4;
            float[] rects = keys.letters;
            if (x >= rects[slot] && x < rects[slot + 2] && y >= rects[slot + 1] && y < rects[slot + 3])
                return (char) ('a' + letter);
        }
        return 0;
    }

    private void follow(KeyboardKeyArea area, MotionEvent event) {
        int index = event.findPointerIndex(pointerId);
        if (index < 0) return;
        for (int sample = 0; sample < event.getHistorySize(); sample++) {
            add(event.getHistoricalX(index, sample), event.getHistoricalY(index, sample),
                event.getHistoricalEventTime(sample));
        }
        float x = event.getX(index);
        float y = event.getY(index);
        add(x, y, event.getEventTime());
        if (!gliding && GlideTypingPolicy.starts(downLetter, letterAt(x, y), downX, x, keys.keyWidth))
            startGlide(area);
    }

    private void add(float x, float y, long eventTime) {
        if (!Float.isFinite(x) || !Float.isFinite(y)) return;
        if (count >= GlideTypingPolicy.MAX_BUFFER_POINTS) {
            int kept = GlideTypingPolicy.MAX_BUFFER_POINTS / 2;
            int[] picked = GlideTypingPolicy.sample(count, kept);
            float[] keptXs = new float[kept];
            float[] keptYs = new float[kept];
            long[] keptTimes = new long[kept];
            for (int index = 0; index < kept; index++) {
                int source = picked[index];
                keptXs[index] = xs[source];
                keptYs[index] = ys[source];
                keptTimes[index] = times[source];
            }
            xs = keptXs;
            ys = keptYs;
            times = keptTimes;
            count = kept;
        }
        if (count == xs.length) {
            xs = Arrays.copyOf(xs, count * 2);
            ys = Arrays.copyOf(ys, count * 2);
            times = Arrays.copyOf(times, count * 2);
        }
        xs[count] = x;
        ys[count] = y;
        times[count] = BoundsPolicy.nonNegative(eventTime - downTime);
        count++;
        if (gliding && trail != null) trail.lineTo(x + trailOffsetX, y + trailOffsetY);
    }

    /** 接管这一次触摸：按键区随后给键发 CANCEL；在按键气泡的覆盖层上画出到此为止的轨迹。 */
    private void startGlide(KeyboardKeyArea area) {
        gliding = true;
        owned = true;
        FrameLayout layer = s.imeLetterRows.keyPreviewLayer;
        if (layer == null) return;
        if (trail == null || trail.getParent() != layer) {
            trail = new GlideTrailView(s);
            layer.addView(trail, LayoutPolicy.frameMatchParentParams());
        }
        area.getLocationInWindow(areaLocation);
        layer.getLocationInWindow(trailLocation);
        trailOffsetX = areaLocation[0] - trailLocation[0];
        trailOffsetY = areaLocation[1] - trailLocation[1];
        trail.start(Color.parseColor(s.skin.accent()), s.pixels(TRAIL_WIDTH_DP),
            xs[0] + trailOffsetX, ys[0] + trailOffsetY);
        for (int point = 1; point < count; point++) trail.lineTo(xs[point] + trailOffsetX, ys[point] + trailOffsetY);
    }

    /** 抬手：补上最后一点，把请求交给服务。 */
    private void finish(MotionEvent event, int index) {
        add(event.getX(index), event.getY(index), event.getEventTime());
        gliding = false;
        if (trail != null) trail.clear();
        if (count < 2) return;
        float[] centers = new float[LETTERS * 2];
        for (int letter = 0; letter < LETTERS; letter++) {
            int slot = letter * 4;
            centers[letter * 2] = (keys.letters[slot] + keys.letters[slot + 2]) / 2;
            centers[letter * 2 + 1] = (keys.letters[slot + 1] + keys.letters[slot + 3]) / 2;
        }
        s.glide(GlideTypingPolicy.request(centers, keys.keyWidth, keys.keyHeight, xs, ys, times, count));
    }

    private void reset() {
        tracking = false;
        gliding = false;
        owned = false;
        count = 0;
        if (trail != null) trail.clear();
    }
}
