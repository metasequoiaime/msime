package app.msime.android;

import android.graphics.Color;
import android.os.Build;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowManager;
import app.msime.android.core.InputViewValuePolicy;

/**
 * 在应用的输入框上直接用手写笔书写（#7080）：系统在 Android 14 起问输入法要不要接一次手写笔书写，接下后笔迹画在系统给的书写窗口里，停笔后整行识别、直接上屏第一候选。接不接、等多久、笔迹怎样变成识别请求见 {@link StylusHandwritingPolicy}。
 *
 * <p>识别器与键盘里的手写面板各用一个，同样是 ML Kit 的中文模型：面板的识别器跟着面板建和关，这里的在第一次书写时建，进程退出时关。模型没下载时这里不下载，提示用户去手写面板下载，与面板的说法一致。光标前的上文只在内存里交给识别器，不保存、不记录。
 */
final class ImeStylusHandwriting {
    private final MSIMEInputService s;
    private final HandwritingRequestTracker requests = new HandwritingRequestTracker();
    private HandwritingRecognizer recognizer;
    private StylusInkView inkView;
    private Window inkWindow;
    /** 已经交给 WindowManager 铺满屏幕的书写窗口。 */
    private Window filledWindow;
    private Runnable recognitionTask;
    private Runnable hintTask;
    /** 每次离开输入框加一；排着的识别和迟到的结果拿它判断是不是还在同一个输入框里。 */
    private long inputGeneration;
    private long hintGeneration = -1;
    /** 正在等模型检查的那次识别，以及从什么时候开始等（uptime）。 */
    private HandwritingRequestTracker.Token checkingToken;
    private long checkingSince;
    private final long serviceDebounce;
    private HandwritingPreferences preferences = HandwritingPreferences.defaults();

    ImeStylusHandwriting(MSIMEInputService s, long serviceDebounce) {
        this.s = s;
        this.serviceDebounce = serviceDebounce;
    }

    /**
     * 书写可能要开始了（{@code onPrepareStylusHandwriting}：笔悬停在屏幕上，或系统问要不要接之前）。先把识别器建好、开始查模型：建 ML Kit 识别器要些时间，放在 {@link #start} 里会拖慢给系统的答复。本版本没有手写时什么也不建。
     */
    void prepare() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE
                || !KeyboardScheme.HANDWRITING.offeredBy(s.edition)) return;
        recognizer();
    }

    /** 系统问要不要接这次书写（{@code onStartStylusHandwriting}）；返回是否接下。 */
    boolean start() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE) return false;
        Window window = s.getStylusHandwritingWindow();
        if (window == null || s.connection == null) return false;
        HandwritingRecognizer current = recognizer();
        StylusHandwritingPolicy.Decision decision = StylusHandwritingPolicy.start(
            KeyboardScheme.HANDWRITING.offeredBy(s.edition),
            EditorPolicy.useEngine(s.editorInputType), chineseInput(), current.availability(),
            hintGeneration == inputGeneration);
        if (decision == StylusHandwritingPolicy.Decision.DECLINE) return false;
        attach(window);
        fillScreen(window);
        android.graphics.Rect screen = s.getSystemService(WindowManager.class).getMaximumWindowMetrics().getBounds();
        inkView.setArea(screen.width(), screen.height());
        preferences = s.imeLayoutRows.readHandwritingPreferences();
        Integer chosen = preferences.inkColor();
        // 笔迹画在应用的界面上而不是键盘上，键帽文字色（面板里的「跟随皮肤」）在深色皮肤下是白的，浅色应用里看不见；跟随皮肤时改用皮肤的强调色。
        inkView.setInk(chosen != null ? chosen : Color.parseColor(s.handwritingSkin.accent()),
            s.pixels(preferences.strokeWidth()));
        s.setStylusHandwritingSessionTimeout(
            java.time.Duration.ofMillis(StylusHandwritingPolicy.SESSION_IDLE_TIMEOUT_MILLIS));
        if (decision == StylusHandwritingPolicy.Decision.HINT) {
            showHint(StylusHandwritingPolicy.hint(current.availability()), true);
            return true;
        }
        cancelHint();
        inkView.showHint(null);
        inkView.setAcceptsInk(true);
        return true;
    }

    /** 会话里的一个手写笔事件（{@code onStylusHandwritingMotionEvent}），直接交给墨迹视图，见 {@link StylusInkView}。 */
    void motion(MotionEvent event) {
        if (inkView != null) inkView.handle(event);
    }

    /** 会话结束（{@code onFinishStylusHandwriting}），书写窗口已经被系统隐藏。还没识别的笔迹照常按计时识别、上屏，下次书写接着这些笔迹写。 */
    void finished() {
        if (hintTask != null) {
            cancelHint();
            if (inkView != null) inkView.clear();
        }
    }

    /** 离开输入框：作废排着的识别和迟到的结果，丢掉没上屏的笔迹。 */
    void inputFinished() {
        inputGeneration++;
        invalidateRecognition();
        cancelHint();
        if (inkView != null) inkView.clear();
    }

    void shutdown() {
        inputFinished();
        if (recognizer != null) {
            try { recognizer.close(); } catch (RuntimeException ignored) { }
        }
        recognizer = null;
    }

    /** 能用的识别器：模型没下载或检查失败时重建一个重新查，用户可能刚在手写面板里下载完。 */
    private HandwritingRecognizer recognizer() {
        if (recognizer != null) {
            HandwritingRecognizer.Availability availability = recognizer.availability();
            if (availability == HandwritingRecognizer.Availability.READY
                    || availability == HandwritingRecognizer.Availability.DOWNLOADING) return recognizer;
            try { recognizer.close(); } catch (RuntimeException ignored) { }
        }
        recognizer = HandwritingRecognizerFactory.create(s);
        return recognizer;
    }

    private boolean chineseInput() {
        String localMode = s.view == null ? "none" : InputViewValuePolicy.textOr(s.view, "local_mode", "none");
        return !s.dedicatedEnglish && "none".equals(localMode)
            && (s.selectedScheme == null || !s.selectedScheme.otherLanguage());
    }

    /** 系统给的书写窗口换过（闲置五分钟或换了应用后会重建）时，把墨迹视图挂到新窗口上。 */
    private void attach(Window window) {
        if (window == inkWindow && inkView != null) return;
        invalidateRecognition();
        inkView = new StylusInkView(s);
        inkView.setListener(new StylusInkView.Listener() {
            @Override public void onStrokeBegan() {
                invalidateRecognition();
            }

            @Override public void onInkChanged(long revision,
                    java.util.List<java.util.List<HandwritingInk.Point>> strokes) {
                inkChanged(revision, strokes);
            }
        });
        window.setContentView(inkView);
        inkWindow = window;
    }

    /**
     * 书写窗口铺满屏幕。系统建窗口时设的是铺满，但窗口用的是输入法的主题（`Theme.DeviceDefault.InputMethod` 是浮动窗口），装 decor 时被改回 wrap_content，API 35 模拟器上它只占 [120,0][960,2400]，两边的笔迹和提示画不出来。
     *
     * <p>系统问我们之前就把 decor 加到了 WindowManager 上，所以改了尺寸要再交给 WindowManager。判断加没加用 decor 有没有 parent（`addView` 时同步挂上 ViewRootImpl），不用 `isAttachedToWindow`：后者要等第一次 traversal，第一次书写时还是 false，跳过更新后本地属性已是铺满，之后就再也不会更新。
     */
    private void fillScreen(Window window) {
        if (window == filledWindow) return;
        window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT);
        View decor = window.peekDecorView();
        // 还没加上时，系统稍后按窗口此刻的属性（已是铺满）加，下次书写再确认一遍。
        if (decor == null || decor.getParent() == null) return;
        s.getSystemService(WindowManager.class).updateViewLayout(decor, window.getAttributes());
        filledWindow = window;
    }

    private void inkChanged(long revision, java.util.List<java.util.List<HandwritingInk.Point>> strokes) {
        invalidateRecognition();
        if (strokes.isEmpty()) return;
        HandwritingRequestTracker.Token token = requests.begin(inputGeneration, revision);
        schedule(token, StylusHandwritingPolicy.recognitionDelayMillis(preferences, serviceDebounce, strokes.size()));
    }

    private void schedule(HandwritingRequestTracker.Token token, long delay) {
        Runnable task = new Runnable() {
            @Override public void run() {
                if (recognitionTask != this) return;
                recognitionTask = null;
                recognize(token);
            }
        };
        recognitionTask = task;
        s.main.postDelayed(task, delay);
    }

    private boolean accepts(HandwritingRequestTracker.Token token) {
        return inkView != null && requests.accepts(token, inputGeneration, inkView.revision(), true);
    }

    private void recognize(HandwritingRequestTracker.Token token) {
        if (!accepts(token) || recognizer == null) return;
        HandwritingRecognizer.Availability availability = recognizer.availability();
        if (availability == HandwritingRecognizer.Availability.DOWNLOADING) {
            long now = android.os.SystemClock.uptimeMillis();
            if (checkingToken != token) {
                checkingToken = token;
                checkingSince = now;
            }
            if (now - checkingSince < StylusHandwritingPolicy.CHECK_TIMEOUT_MILLIS) {
                schedule(token, StylusHandwritingPolicy.AVAILABILITY_POLL_MILLIS);
                return;
            }
            // 检查迟迟不返回（API 35 模拟器上 ML Kit 的模型检查要几十秒）：丢掉笔迹说一声，查完以后照常能写，所以不算这个输入框提示过。
            inkView.clear();
            showHint(StylusHandwritingPolicy.hint(availability), false);
            return;
        }
        if (availability != HandwritingRecognizer.Availability.READY) {
            // 接下时还在检查，查完才知道模型没下载：丢掉这些笔迹，提示一次，结束会话。
            inkView.clear();
            showHint(StylusHandwritingPolicy.hint(availability), true);
            return;
        }
        HandwritingRecognizer.Request request;
        try {
            request = StylusHandwritingPolicy.request(token.revision(), inkView.strokes(),
                s.pixels(8), s.pixels(64));
        } catch (IllegalArgumentException invalid) {
            request = null;
        }
        if (request == null) {
            inkView.clear();
            return;
        }
        final HandwritingRecognizer expected = recognizer;
        try {
            recognizer.setPreContext(HandwritingRecognizer.preContext(s.connection));
            recognizer.recognize(request, new HandwritingRecognizer.RecognitionListener() {
                @Override public void onResult(long revision, java.util.List<String> values) {
                    s.main.post(() -> {
                        if (recognizer != expected || revision != token.revision() || !accepts(token)) return;
                        commit(HandwritingRecognizer.sanitizeCandidates(values));
                    });
                }

                @Override public void onFailure(long revision) {
                    s.main.post(() -> {
                        if (recognizer != expected || revision != token.revision() || !accepts(token)) return;
                        inkView.clear();
                    });
                }
            });
        } catch (RuntimeException error) {
            if (accepts(token)) inkView.clear();
        }
    }

    /** 上屏第一候选，与手写面板确认候选时相同：先按首选结束引擎里的组字，再以手写来源上屏，简繁和统计照常。 */
    private void commit(java.util.List<String> candidates) {
        if (candidates.isEmpty() || s.connection == null) {
            inkView.clear();
            return;
        }
        long expectedGeneration = inputGeneration;
        if (s.hasEngineComposition()) s.command(MSIMEInputService.FINISH_COMPOSITION_COMMAND);
        if (expectedGeneration != inputGeneration || s.connection == null) return;
        s.commitText(s.chineseOutput(candidates.get(0), s.view), TypingSource.HANDWRITING);
        inkView.clear();
        s.render();
    }

    private void invalidateRecognition() {
        if (recognitionTask != null) s.main.removeCallbacks(recognitionTask);
        recognitionTask = null;
        requests.invalidate();
        if (recognizer != null) {
            try { recognizer.cancelPending(); } catch (RuntimeException ignored) { }
        }
    }

    /**
     * 不记笔迹，只在落笔处显示提示，过一会儿结束会话。会话的闲置计时从此刻重新算，免得提示还没看清系统就把书写窗口收了。
     *
     * @param once 是否算这个输入框提示过：模型没下载时算，之后这个输入框里不再接手写笔书写
     */
    private void showHint(String text, boolean once) {
        cancelHint();
        if (once) hintGeneration = inputGeneration;
        if (inkView == null) return;
        inkView.setAcceptsInk(false);
        inkView.showHint(text);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            s.setStylusHandwritingSessionTimeout(
                java.time.Duration.ofMillis(StylusHandwritingPolicy.SESSION_IDLE_TIMEOUT_MILLIS));
        }
        Runnable task = new Runnable() {
            @Override public void run() {
                if (hintTask != this) return;
                hintTask = null;
                if (inkView != null) inkView.showHint(null);
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) s.finishStylusHandwriting();
            }
        };
        hintTask = task;
        s.main.postDelayed(task, StylusHandwritingPolicy.HINT_MILLIS);
    }

    private void cancelHint() {
        if (hintTask != null) s.main.removeCallbacks(hintTask);
        hintTask = null;
    }
}
