package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.view.View;
import android.widget.FrameLayout;

/** 展开候选的固定画布，滚动只作用于里面的候选内容。 */
public final class CandidateCanvasFrame extends FrameLayout {
    private final CandidateCanvasRenderer renderer = new CandidateCanvasRenderer(this);
    private int surface;

    public CandidateCanvasFrame(Context context) { super(context); }

    public void apply(CandidateCanvasAssets assets, boolean active, int surface) {
        this.surface = surface;
        renderer.set(assets, active);
        float density = getResources().getDisplayMetrics().density;
        int[] padding = new int[4];
        if (active && assets != null) for (int i = 0; i < 4; i++)
            padding[i] = Math.round((float) (assets.outer[i] + assets.content[i]) * density);
        if (getPaddingLeft() != padding[3] || getPaddingTop() != padding[0]
                || getPaddingRight() != padding[1] || getPaddingBottom() != padding[2])
            setPadding(padding[3], padding[0], padding[1], padding[2]);
        setVisibility(active ? VISIBLE : GONE);
    }

    @Override protected void dispatchDraw(Canvas canvas) {
        renderer.before(canvas, surface);
        super.dispatchDraw(canvas);
        renderer.after(canvas);
    }
    @Override protected void onVisibilityChanged(View changed, int visibility) {
        super.onVisibilityChanged(changed, visibility);
        if (renderer != null) renderer.visible(visibility == VISIBLE);
    }
    @Override protected void onWindowVisibilityChanged(int visibility) {
        super.onWindowVisibilityChanged(visibility);
        if (renderer != null) renderer.visible(visibility == VISIBLE);
    }
    @Override protected void onAttachedToWindow() { super.onAttachedToWindow(); renderer.attached(); }
    @Override protected void onDetachedFromWindow() { renderer.detached(); super.onDetachedFromWindow(); }
}
