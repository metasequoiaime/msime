import app.msime.android.HandwritingInk;
import app.msime.android.HandwritingPreferences;
import app.msime.android.HandwritingRecognizer;
import app.msime.android.StylusHandwritingPolicy;
import app.msime.android.StylusHandwritingPolicy.Decision;
import java.util.ArrayList;
import java.util.List;

/** 输入框上的手写笔书写（#7080）：接不接、停笔多久识别、屏幕笔迹到识别请求的换算。 */
public final class StylusHandwritingPolicySmoke {
    private static final HandwritingRecognizer.Availability READY = HandwritingRecognizer.Availability.READY;
    private static final HandwritingRecognizer.Availability CHECKING = HandwritingRecognizer.Availability.DOWNLOADING;
    private static final HandwritingRecognizer.Availability MISSING = HandwritingRecognizer.Availability.DOWNLOAD_REQUIRED;
    private static final HandwritingRecognizer.Availability ABSENT = HandwritingRecognizer.Availability.UNAVAILABLE;

    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        start();
        delay();
        request();
        System.out.println("Android stylus handwriting policy: start decisions, recognition delay and ink-to-request conversion passed");
    }

    private static void start() {
        check(StylusHandwritingPolicy.start(true, true, true, READY, false) == Decision.ACCEPT, "ready model accepts");
        // 识别器刚建好还在查模型：先接下记笔迹，识别时再看。
        check(StylusHandwritingPolicy.start(true, true, true, CHECKING, false) == Decision.ACCEPT, "checking accepts");
        check(StylusHandwritingPolicy.start(true, true, true, CHECKING, true) == Decision.ACCEPT,
            "an earlier hint does not block a model that may now be ready");
        check(StylusHandwritingPolicy.start(true, true, true, MISSING, false) == Decision.HINT, "missing model hints");
        check(StylusHandwritingPolicy.start(true, true, true, ABSENT, false) == Decision.HINT, "absent recognizer hints");
        check(StylusHandwritingPolicy.start(true, true, true, MISSING, true) == Decision.DECLINE, "hint once per editor");
        check(StylusHandwritingPolicy.start(true, true, true, ABSENT, true) == Decision.DECLINE, "hint once per editor");
        for (HandwritingRecognizer.Availability availability : HandwritingRecognizer.Availability.values()) {
            check(StylusHandwritingPolicy.start(false, true, true, availability, false) == Decision.DECLINE,
                "editions without handwriting never take the stylus");
            check(StylusHandwritingPolicy.start(true, false, true, availability, false) == Decision.DECLINE,
                "password and number fields never take the stylus");
            check(StylusHandwritingPolicy.start(true, true, false, availability, false) == Decision.DECLINE,
                "English and other-language input never take the stylus");
        }
        check(StylusHandwritingPolicy.start(true, true, true, null, false) == Decision.DECLINE, "no recognizer state");
        // 与手写面板状态行同一套说法。
        check(StylusHandwritingPolicy.hint(MISSING).startsWith("首次下载后可离线手写"), "missing-model hint");
        check(StylusHandwritingPolicy.hint(ABSENT).equals("此构建不含手写识别"), "absent-recognizer hint");
        check(StylusHandwritingPolicy.hint(CHECKING).startsWith("正在检查中文手写模型"), "checking hint");
        check(StylusHandwritingPolicy.SESSION_IDLE_TIMEOUT_MILLIS > HandwritingPreferences.DELAY_MAX,
            "the session outlives the longest recognition wait");
        // 识别等待加上等模型检查的上限仍在会话里，「还在检查」的提示出来时书写窗口还在屏幕上。
        check(HandwritingPreferences.DELAY_MAX + StylusHandwritingPolicy.CHECK_TIMEOUT_MILLIS
                < StylusHandwritingPolicy.SESSION_IDLE_TIMEOUT_MILLIS,
            "the checking hint appears while the session is still alive");
        check(StylusHandwritingPolicy.HINT_MILLIS < StylusHandwritingPolicy.SESSION_IDLE_TIMEOUT_MILLIS,
            "a hint ends the session before the restarted idle timer does");
    }

    private static void delay() {
        long debounce = 550;
        check(StylusHandwritingPolicy.recognitionDelayMillis(HandwritingPreferences.defaults(), debounce, 1) == debounce,
            "default wait is the service debounce");
        check(StylusHandwritingPolicy.recognitionDelayMillis(preferences(1200), debounce, 3) == 1200,
            "a longer preference extends the wait");
        check(StylusHandwritingPolicy.recognitionDelayMillis(preferences(200), debounce, 3) == debounce,
            "a shorter preference cannot undercut the debounce");
        check(StylusHandwritingPolicy.recognitionDelayMillis(preferences(1500), debounce, HandwritingInk.MAX_STROKES) == 0,
            "a full ink buffer recognizes at once");
    }

    private static void request() {
        check(StylusHandwritingPolicy.request(1, List.of(), 8, 64) == null, "no ink, no request");

        // 屏幕中间写的两笔：裁到外接框、四周留 8、居中。
        List<List<HandwritingInk.Point>> strokes = List.of(
            List.of(point(1000, 700, 10), point(1100, 700, 20)),
            List.of(point(1050, 650, 30), point(1050, 760, 40)));
        HandwritingRecognizer.Request request = StylusHandwritingPolicy.request(7, strokes, 8, 64);
        check(request.revision() == 7, "revision kept");
        check(request.width() == 116 && request.height() == 126, "area is the padded bounding box: " + request.width() + "x" + request.height());
        HandwritingInk.Point first = request.strokes().get(0).get(0);
        check(first.x() == 8 && first.y() == 58, "ink is offset into the area: " + first);
        HandwritingInk.Point last = request.strokes().get(1).get(1);
        check(last.x() == 58 && last.y() == 118 && last.timeMillis() == 40, "order and time kept: " + last);
        check(request.strokes().size() == 2 && request.strokes().get(1).size() == 2, "stroke structure kept");

        // 单写一横：外接框几乎没有高度，书写区域至少 64 高，笔迹在正中。
        HandwritingRecognizer.Request flat = StylusHandwritingPolicy.request(1,
            List.of(List.of(point(300, 400, 1), point(420, 401, 2))), 8, 64);
        check(flat.width() == 136 && flat.height() == 64, "minimum side for a flat stroke: " + flat.width() + "x" + flat.height());
        HandwritingInk.Point flatStart = flat.strokes().get(0).get(0);
        check(flatStart.x() == 8 && flatStart.y() == 31.5f, "flat stroke centered vertically: " + flatStart);

        // 一个点：宽高都取最小边，点在中心。
        HandwritingRecognizer.Request dot = StylusHandwritingPolicy.request(1,
            List.of(List.of(point(50, 60, 1))), 8, 64);
        check(dot.width() == 64 && dot.height() == 64, "dot area");
        check(dot.strokes().get(0).get(0).x() == 32 && dot.strokes().get(0).get(0).y() == 32, "dot centered");

        // 横跨整块大屏的一行：加上留白超过 4096 时等比缩小，仍在区域内。
        HandwritingRecognizer.Request wide = StylusHandwritingPolicy.request(1,
            List.of(List.of(point(0, 100, 1), point(4096, 300, 2))), 32, 64);
        check(wide.width() <= 4096 && wide.height() <= 4096, "area within recognizer bounds");
        float scale = 4096f / (4096 + 64);
        check(Math.abs(wide.height() - (200 + 64) * scale) < 0.01, "height scaled with width: " + wide.height());
        for (List<HandwritingInk.Point> stroke : wide.strokes()) {
            for (HandwritingInk.Point point : stroke) {
                check(point.x() >= 0 && point.x() <= wide.width() && point.y() >= 0 && point.y() <= wide.height(),
                    "scaled point inside area: " + point);
            }
        }
        check(Math.abs(wide.strokes().get(0).get(1).x() - (4096 + 32) * scale) < 0.01, "scaled x keeps proportion");

        // 识别器的笔画上限内的输入都能成请求，请求还是不可变快照。
        List<List<HandwritingInk.Point>> many = new ArrayList<>();
        for (int index = 0; index < HandwritingInk.MAX_STROKES; index++) {
            many.add(List.of(point(index * 10, 5, index), point(index * 10 + 5, 25, index + 1)));
        }
        HandwritingRecognizer.Request full = StylusHandwritingPolicy.request(2, many, 8, 64);
        check(full.strokes().size() == HandwritingInk.MAX_STROKES, "full ink converts");
        try {
            full.strokes().get(0).add(point(1, 1, 1));
            throw new AssertionError("request strokes must be immutable");
        } catch (UnsupportedOperationException expected) { }
    }

    private static HandwritingPreferences preferences(int delay) {
        return HandwritingPreferences.of("line", delay, false, "follow_skin", 3);
    }

    private static HandwritingInk.Point point(float x, float y, long time) {
        return new HandwritingInk.Point(x, y, time);
    }
}
