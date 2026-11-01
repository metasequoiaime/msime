package app.msime.android;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.view.View;

/** 水杉娘的耳部聆听界面；红色信号只随真实采样音量变化，不使用循环动画。 */
public final class OfflineVoiceListeningView extends View {
    private final VoiceMascotDrawing drawing = new VoiceMascotDrawing();
    private Bitmap mascot;
    private float level;

    public OfflineVoiceListeningView(Context context) {
        super(context);
        setContentDescription("水杉娘正在聆听，耳边的红色信号随说话音量变化");
        setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_YES);
    }

    public void setMascot(Bitmap value) {
        mascot = value;
        invalidate();
    }

    public void setLevel(float value) {
        level = Float.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0;
        invalidate();
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        drawing.draw(canvas, mascot, getWidth(), getHeight(), level);
    }
}
