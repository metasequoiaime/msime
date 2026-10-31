package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;

/** 普通候选条与展开面板共用的联系人右上角标，不改变候选文本和提交身份。 */
public final class ContactCandidateButton extends KeyboardPressButton {
    private final Paint badgePaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private boolean contact;

    public ContactCandidateButton(Context context) {
        super(context);
        setAllCaps(false);
        setStateListAnimator(null);
        setAnimatesPressFeedback(false);
    }

    public void setContactSource(int source) {
        boolean next = ContactCandidatePolicy.isContact(source);
        if (contact == next) return;
        contact = next;
        requestLayout();
        invalidate();
    }

    @Override public int getCompoundPaddingRight() {
        return super.getCompoundPaddingRight() + (contact ? dp(13) : 0);
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (!contact) return;
        badgePaint.setColor(getCurrentTextColor());
        badgePaint.setStyle(Paint.Style.FILL);
        float right = getWidth() - dp(3), top = dp(3), size = dp(9);
        float center = right - size / 2;
        canvas.drawCircle(center, top + size * .23f, size * .21f, badgePaint);
        canvas.drawRoundRect(right - size, top + size * .53f, right, top + size,
            size * .3f, size * .3f, badgePaint);
    }

    private int dp(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }
}
