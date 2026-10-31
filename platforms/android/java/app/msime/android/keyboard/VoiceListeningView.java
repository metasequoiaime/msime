package app.msime.android;


import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.os.SystemClock;
import android.text.Layout;
import android.text.StaticLayout;
import android.text.TextPaint;
import android.text.TextUtils;
import android.widget.TextView;

/**
 * 键盘内的语音聆听面板：工具栏入口用水杉娘与耳部音量信号，长按入口用角色缩略图和红色音量短线，下面是标题和如何结束录音的提示。排布、平滑和文字裁剪在 {@link VoiceListeningPolicy}，iOS 与 HarmonyOS 用同一组数值。
 *
 * <p>节点 text 是标题（「正在聆听…」「正在识别…」「识别完成」），提示由调用方更新；结束采集与取消由面板下方的显式按钮处理。颜色由调用方从皮肤传入。
 *
 * <p>识别器报来中间文字时（{@link #setPartial}），聆听中标题位换成这段文字，最多两行、放不下从开头省略；结束录音后标题回到「正在识别…」，中间文字挪到提示行。光圈只在音量变化、还没到位时逐帧重绘，系统关掉动画时停在安静时的样子。
 *
 * <p>按住空格说话时手指滑进取消区，面板换成「松开取消」的样子（{@link #setReleaseToCancel}）：标题改成「松开取消」，圆盘和光圈改用提示色，滑回去即恢复。
 */
public final class VoiceListeningView extends TextView {
    /** 圆盘的最大直径；面板矮时按 {@link VoiceListeningPolicy#plan} 缩小。 */
    public static final float ORB_DP = VoiceListeningPolicy.ORB_MAX_DP;
    /** 麦克风占圆盘直径的比例：72 dp 的圆盘上是 32 dp，64 dp 上约 28 dp。 */
    public static final float MIC_RATIO = 32f / 72f;
    public static final float TITLE_SP = 16f;
    public static final float HINT_SP = 13f;
    /** 识别中没有中间文字时提示行的文案，与 iOS 一致。 */
    static final String RECOGNIZING_HINT = "识别完成后自动上屏";

    private final VoiceMascotDrawing mascotDrawing = new VoiceMascotDrawing();
    private Bitmap toolbarMascot;
    private boolean mascotEnabled;
    private final Paint bitmapPaint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
    private final RectF mascotBounds = new RectF();
    private final Paint orb = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint halo = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint icon = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final TextPaint title = new TextPaint(Paint.ANTI_ALIAS_FLAG);
    private final TextPaint hint = new TextPaint(Paint.ANTI_ALIAS_FLAG);
    private int accent = Color.BLUE;
    private int onAccent = Color.WHITE;
    private int secondary = Color.GRAY;
    private boolean releaseToCancel;
    private boolean holdMode;
    private Bitmap mascot;
    private CharSequence listeningDescription;
    /** 识别器报来的音量（目标）和此刻画出来的音量。 */
    private float levelTarget;
    private float levelShown;
    /** 上一帧推进音量的时刻；为 0 时下一帧按一帧的时长推进。 */
    private long levelFrameAt;
    private String hintText = "识别结束后自动上屏";
    /** 最新的中间文字，已规整空白。 */
    private String partial = "";
    /** 已经结束录音、在等识别结果。 */
    private boolean recognizing;
    private boolean waitingForSend;
    /** 结果已上屏：提示行显示「已识别：」加结果。 */
    private String doneText;
    private String fittedTitleSource;
    private String[] fittedTitle = new String[0];
    private String fittedHintSource;
    private String fittedHint = "";
    private float fittedWidth = Float.NaN;
    private float fittedTitleSize = Float.NaN;
    private float fittedHintSize = Float.NaN;
    private int fittedLines;

    public VoiceListeningView(Context context) {
        super(context);
        setText("正在聆听…");
        setContentDescription("正在聆听，识别结束后自动上屏");
        setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_YES);
        ViewPolicy.clearBackground(this);
        title.setTextAlign(Paint.Align.CENTER);
        hint.setTextAlign(Paint.Align.CENTER);
    }

    /** 工具栏会话可显示人物；迟到素材只更新工具栏绘制，不覆盖长按面板的角色。 */
    public void setMascotEnabled(boolean value) {
        mascotEnabled = value;
        invalidate();
    }

    public void setMascot(Bitmap value) {
        toolbarMascot = value;
        invalidate();
    }

    /** accent 圆盘、圆盘上的麦克风色、标题色（kbFg）、提示色（kbSub）。 */
    public void setColors(int accentColor, int onAccentColor, int foreground, int secondary) {
        accent = accentColor;
        onAccent = onAccentColor;
        title.setColor(foreground);
        hint.setColor(secondary);
        this.secondary = secondary;
        invalidate();
    }

    /** 长按入口复用角色缩略图，音量由两侧红色短线表示，静音时不画短线。 */
    public void setHoldMode(Bitmap image) {
        holdMode = true;
        mascot = image;
        setText("当家的，你说吧！我听着呢。");
        invalidate();
    }

    /** 按住说话时手指是否在取消区：在时标题显示「松开取消」、圆盘改用提示色，节点描述同步；离开时恢复原来的标题与描述。 */
    public void setReleaseToCancel(boolean value) {
        if (releaseToCancel == value) return;
        releaseToCancel = value;
        if (value) {
            listeningDescription = getContentDescription();
            setContentDescription("松开取消");
        } else {
            setContentDescription(listeningDescription);
            listeningDescription = null;
        }
        invalidate();
    }

    /** 替换提示行的说明（如何结束录音）。 */
    public void setHint(String value) {
        hintText = TextPolicy.emptyIfNull(value);
        invalidate();
    }

    /** 识别器报来的中间文字：聆听中显示在标题位，识别中显示在提示行。 */
    public void setPartial(String value) {
        String next = VoiceListeningPolicy.normalized(value);
        if (next.equals(partial)) return;
        partial = next;
        if (!recognizing) {
            CharSequence description = partial.isEmpty() ? "正在聆听" : "正在聆听：" + partial;
            if (releaseToCancel) listeningDescription = description;
            else setContentDescription(description);
        }
        invalidate();
    }

    /** 已经结束录音：标题换成 {@code message}（「正在识别…」「正在润色…」），中间文字挪到提示行，光圈收回。 */
    public void showRecognizing(String message) {
        recognizing = true;
        setText(message);
        resetLevel();
        invalidate();
    }

    /** 服务自行结束时保留文字等待发送，不再显示正在采集。 */
    public void showWaitingForSend(String text) {
        recognizing = true;
        waitingForSend = true;
        if (TextPolicy.hasText(text)) setPartial(text);
        setText("录音已结束");
        resetLevel();
        invalidate();
    }

    /** 结果已上屏：标题「识别完成」，提示行「已识别：」加结果（放不下从开头省略）。 */
    public void showDone(String text) {
        recognizing = true;
        doneText = VoiceListeningPolicy.normalized(text);
        setText("识别完成");
        setContentDescription("已识别：" + text);
        resetLevel();
        invalidate();
    }

    /** 识别器报来的音量（0–1，见 {@link PlatformSpeechPolicy#level} 和 {@link VoiceAudioLevel#fromPcm}）；光圈按 {@link VoiceListeningPolicy#step} 平滑地跟上。 */
    public void setLevel(float value) {
        float next = VoiceListeningPolicy.clamp(value);
        // 结束录音后采集线程可能还送来最后几块的音量，光圈不再跟着涨。
        if (recognizing && next > 0f) return;
        if (next == levelTarget) return;
        levelTarget = next;
        if (!VoiceListeningPolicy.settled(levelShown, levelTarget)) postInvalidateOnAnimation();
    }

    /** 识别器不再报音量（说完了、出结果了）：光圈按回落速度收回到安静时的样子。 */
    public void resetLevel() {
        setLevel(0f);
    }

    /** 光圈此刻的显示音量，供设备验收读取。 */
    public float shownLevel() {
        return levelShown;
    }

    /** 系统设置里关掉了动画（开发者选项「动画时长缩放」为 0、无障碍「移除动画」）：光圈不跟音量动。 */
    private static boolean motionEnabled() {
        return ValueAnimator.areAnimatorsEnabled();
    }

    @Override protected void onDetachedFromWindow() {
        levelFrameAt = 0;
        super.onDetachedFromWindow();
    }

    @Override protected void onDraw(Canvas canvas) {
        float scale = DimensionPolicy.density(getContext());
        ViewPolicy.setTextSize(title, KeyboardTypographyPolicy.keySp(getContext(), TITLE_SP));
        ViewPolicy.setTextSize(hint, KeyboardTypographyPolicy.keySp(getContext(), HINT_SP));
        Paint.FontMetrics titleMetrics = title.getFontMetrics();
        Paint.FontMetrics hintMetrics = hint.getFontMetrics();
        float titleLine = titleMetrics.descent - titleMetrics.ascent;
        float hintLine = hintMetrics.descent - hintMetrics.ascent;
        if (holdMode && getHeight() < 130f * scale) {
            drawCompactHold(canvas, scale, titleLine, hintLine, titleMetrics, hintMetrics);
            return;
        }
        VoiceListeningPolicy.Plan plan = VoiceListeningPolicy.plan(getHeight(), titleLine, hintLine, scale);
        advanceLevel();
        float cx = getWidth() / 2f;
        float cy = plan.indicatorCenterY(scale);
        drawIndicator(canvas, cx, cy, plan.orb(), plan.halo(), scale);
        // 文字居中绘制，左右各留 16 dp。标题位预留 plan 的行数，实际只占一行时提示紧跟其后。
        float available = BoundsPolicy.nonNegative(getWidth() - getPaddingLeft() - getPaddingRight()
            - DimensionPolicy.floatPixels(getContext(), 32));
        fitTitle(available, plan.titleLines());
        float baseline = plan.titleTop(scale) - titleMetrics.ascent;
        for (String line : fittedTitle) {
            canvas.drawText(line, cx, baseline, title);
            baseline += titleLine;
        }
        if (!plan.showsHint()) return;
        fitHint(available);
        if (fittedHint.isEmpty()) return;
        float hintTop = plan.titleTop(scale) + Math.max(1, fittedTitle.length) * titleLine
            + VoiceListeningPolicy.GAP_TITLE_HINT_DP * scale;
        canvas.drawText(fittedHint, cx, hintTop - hintMetrics.ascent, hint);
    }

    /** 三行操作区较高时角色与文字并排，保留识别文字和录音状态。 */
    private void drawCompactHold(Canvas canvas, float scale, float titleLine, float hintLine,
                                 Paint.FontMetrics titleMetrics, Paint.FontMetrics hintMetrics) {
        advanceLevel();
        float diameter = Math.max(0f, Math.min(64f * scale, getHeight() - 12f * scale));
        float imageBox = diameter + 36f * scale;
        drawIndicator(canvas, imageBox / 2f, getHeight() / 2f, diameter, 0f, scale);
        float width = Math.max(0f, getWidth() - imageBox - 16f * scale);
        float cx = imageBox + width / 2f;
        fitTitle(width, 1);
        boolean showsHint = getHeight() >= titleLine + hintLine + 12f * scale;
        float used = titleLine + (showsHint ? hintLine + 4f * scale : 0f);
        float top = Math.max(0f, (getHeight() - used) / 2f);
        for (String line : fittedTitle) canvas.drawText(line, cx, top - titleMetrics.ascent, title);
        if (showsHint) {
            fitHint(width);
            canvas.drawText(fittedHint, cx, top + titleLine + 4f * scale - hintMetrics.ascent, hint);
        }
    }

    private void drawIndicator(Canvas canvas, float cx, float cy, float diameter, float haloWidth, float scale) {
        // 素材还没解码出来（或解码失败）时先画原来的麦克风圆盘，素材到达后 setMascot 重绘成人物。
        if (!holdMode && mascotEnabled && toolbarMascot != null && !toolbarMascot.isRecycled()) {
            float box = diameter + 2f * haloWidth;
            float width = Math.min(getWidth(), box * 320f / 250f);
            int saved = canvas.save();
            canvas.translate(cx - width / 2f, cy - box / 2f);
            // 识别阶段立即收起红线；工具栏绘制不参与长按角色与操作区。
            mascotDrawing.draw(canvas, toolbarMascot, width, box, recognizing ? 0f : levelShown);
            canvas.restoreToCount(saved);
            return;
        }
        float radius = diameter / 2f;
        int orbColor = releaseToCancel ? secondary : accent;
        float spread = VoiceListeningPolicy.spread(levelShown, haloWidth);
        if (!holdMode && spread > 0f) {
            halo.setColor(orbColor);
            halo.setAlpha(Math.round(255 * VoiceListeningPolicy.OUTER_ALPHA));
            canvas.drawCircle(cx, cy, radius + spread, halo);
            halo.setAlpha(Math.round(255 * VoiceListeningPolicy.INNER_ALPHA));
            canvas.drawCircle(cx, cy, radius + spread / 2f, halo);
        }
        orb.setColor(orbColor);
        if (holdMode && mascot != null && !mascot.isRecycled()) {
            float factor = Math.min(diameter / mascot.getWidth(), diameter / mascot.getHeight());
            float width = mascot.getWidth() * factor, height = mascot.getHeight() * factor;
            mascotBounds.set(cx - width / 2, cy - height / 2, cx + width / 2, cy + height / 2);
            canvas.drawBitmap(mascot, null, mascotBounds, bitmapPaint);
        } else canvas.drawCircle(cx, cy, radius, orb);
        if (holdMode && levelShown > 0.01f) {
            halo.setColor(Color.rgb(216, 70, 70));
            halo.setAlpha(255);
            halo.setStrokeWidth(2f * scale);
            for (int side = -1; side <= 1; side += 2) for (int at = 0; at < 3; at++) {
                float x = cx + side * (radius + (5 + at * 4) * scale);
                float half = Math.min(radius, (6 + at * 3) * scale) * levelShown;
                canvas.drawLine(x, cy - half, x, cy + half, halo);
            }
        }
        float mic = diameter * MIC_RATIO;
        if (mic > 0f && !(holdMode && mascot != null && !mascot.isRecycled())) {
            KeyboardIconPaths.draw(canvas, icon, KeyboardIconPaths.Icon.MIC, cx - mic / 2f,
                cy - mic / 2f, mic, onAccent);
        }
    }

    /** 按经过的时间把显示音量推向目标；没到位就约下一帧，到位或关了动画就停。 */
    private void advanceLevel() {
        if (!motionEnabled()) {
            levelShown = 0f;
            levelFrameAt = 0;
            return;
        }
        long now = SystemClock.uptimeMillis();
        float elapsed = levelFrameAt == 0 ? 16f : now - levelFrameAt;
        levelShown = VoiceListeningPolicy.step(levelShown, levelTarget, elapsed);
        if (VoiceListeningPolicy.settled(levelShown, levelTarget)) {
            levelShown = VoiceListeningPolicy.clamp(levelTarget);
            levelFrameAt = 0;
        } else {
            levelFrameAt = now;
            postInvalidateOnAnimation();
        }
    }

    /** 标题位的文字：取消区里是「松开取消」，聆听中有中间文字时是它（放不下从开头省略，最多 {@code lines} 行），其余是节点 text（放不下省略结尾）。 */
    private void fitTitle(float available, int lines) {
        boolean heard = !releaseToCancel && (!recognizing || waitingForSend) && !partial.isEmpty();
        String source = releaseToCancel ? "松开取消" : heard ? partial : String.valueOf(getText());
        String key = (heard ? "p:" : "t:") + source;
        if (key.equals(fittedTitleSource) && available == fittedWidth && title.getTextSize() == fittedTitleSize
                && lines == fittedLines) return;
        fittedTitleSource = key;
        fittedWidth = available;
        fittedTitleSize = title.getTextSize();
        fittedLines = lines;
        fittedHintSource = null;
        if (!heard) {
            fittedTitle = new String[] {TextUtils.ellipsize(source, title, available, TextUtils.TruncateAt.END).toString()};
            return;
        }
        String fitted = VoiceListeningPolicy.fitTail(source, text -> lineCount(text, available) <= lines);
        StaticLayout layout = layout(fitted, available);
        String[] out = new String[Math.min(lines, layout.getLineCount())];
        for (int index = 0; index < out.length; index++) {
            out[index] = fitted.substring(layout.getLineStart(index), layout.getLineEnd(index)).trim();
        }
        fittedTitle = out;
    }

    /** 提示行：结果已上屏时是「已识别：」加结果，识别中是中间文字（都从开头省略），否则是说明（省略结尾）。只画一行。 */
    private void fitHint(float available) {
        String prefix = doneText != null ? "已识别：" : "";
        boolean tail = doneText != null || (recognizing && !waitingForSend && !partial.isEmpty());
        String source = waitingForSend ? hintText : doneText != null ? doneText : recognizing ? (partial.isEmpty() ? RECOGNIZING_HINT : partial)
            : hintText;
        String key = (tail ? "p:" : "t:") + prefix + source;
        if (key.equals(fittedHintSource) && available == fittedWidth && hint.getTextSize() == fittedHintSize) return;
        fittedHintSource = key;
        fittedHintSize = hint.getTextSize();
        if (!tail) {
            fittedHint = TextUtils.ellipsize(source, hint, available, TextUtils.TruncateAt.END).toString();
            return;
        }
        float prefixWidth = hint.measureText(prefix);
        fittedHint = prefix + VoiceListeningPolicy.fitTail(source, text -> prefixWidth + hint.measureText(text) <= available);
    }

    private int lineCount(String text, float available) {
        return layout(text, available).getLineCount();
    }

    private StaticLayout layout(String text, float available) {
        int width = Math.max(1, (int) Math.floor(available));
        return StaticLayout.Builder.obtain(text, 0, text.length(), title, width)
            .setAlignment(Layout.Alignment.ALIGN_NORMAL)
            .setIncludePad(false)
            .build();
    }
}
