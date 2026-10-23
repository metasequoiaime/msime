package app.msime.android;

import android.graphics.Color;
import android.graphics.Rect;
import android.view.Gravity;
import android.view.View;
import android.view.ViewTreeObserver;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.PopupWindow;
import android.widget.ScrollView;
import android.widget.TextView;

/** 挂在输入法透明窗口内的全文预览；不取得编辑器焦点，不把正文写入输入框。 */
final class ClipboardTextPreview {
    private final MSIMEInputService service;
    private PopupWindow popup;
    private View pendingAnchor;
    private ViewTreeObserver.OnGlobalLayoutListener pendingLayout;

    ClipboardTextPreview(MSIMEInputService service) {
        this.service = service;
    }

    void show(String text) {
        show(text, null, null);
    }

    /** 动作由记录所属面板提供；只读预览不带插入和修改入口。 */
    void show(String text, Runnable insert, Runnable edit) {
        dismiss();
        View anchor = service.keyboardRoot;
        if (anchor == null || !anchor.isShown() || !anchor.isAttachedToWindow()) return;
        // API 28 会把子窗口的实际画面裁在父窗口内；先扩展透明父窗口，键盘本身保留原高。
        service.setClipboardPreviewExpanded(true);
        pendingAnchor = anchor;
        pendingLayout = () -> {
            if (anchor.isLayoutRequested()) return;
            clearPendingLayout();
            showInWindow(anchor, text, insert, edit);
        };
        anchor.getViewTreeObserver().addOnGlobalLayoutListener(pendingLayout);
        anchor.requestLayout();
    }

    private void showInWindow(View anchor, String text, Runnable insert, Runnable edit) {
        if (!anchor.isShown() || !anchor.isAttachedToWindow()) { dismiss(); return; }
        View keyboard = service.keyboardSurface;
        int gap = service.pixels(12);
        Rect screen = new Rect();
        anchor.getWindowVisibleDisplayFrame(screen);
        int[] position = new int[2];
        keyboard.getLocationOnScreen(position);
        // 预览下沿停在键盘上方，操作行、返回键和卡片仍可直接点按。
        int bottom = position[1] - gap;
        int height = Math.min(service.pixels(560), bottom - screen.top - gap);
        int width = Math.min(keyboard.getWidth(), screen.width()) - gap * 2;
        if (height < service.pixels(96) || width <= 0) { dismiss(); return; }
        int left = Math.max(screen.left + gap,
            Math.min(position[0] + gap, screen.right - gap - width));

        LinearLayout panel = new LinearLayout(service);
        panel.setOrientation(LinearLayout.VERTICAL);
        panel.setAccessibilityPaneTitle("剪贴板全文预览");
        panel.setPadding(gap, gap, gap, gap);
        panel.setBackground(DrawablePolicy.rounded(
            Color.parseColor(service.skin.background()), service.pixels(12)));
        ScrollView scroll = new ScrollView(service);
        scroll.setContentDescription("剪贴板全文滚动区域");
        scroll.setVerticalScrollBarEnabled(true);
        scroll.setScrollbarFadingEnabled(false);
        scroll.setScrollBarStyle(View.SCROLLBARS_INSIDE_INSET);
        TextView body = new TextView(service);
        body.setText(text);
        body.setTextSize(17);
        body.setTextColor(Color.parseColor(service.skin.keyForeground()));
        body.setPadding(0, 0, gap, gap);
        scroll.addView(body, new ScrollView.LayoutParams(
            ScrollView.LayoutParams.MATCH_PARENT, ScrollView.LayoutParams.WRAP_CONTENT));
        panel.addView(scroll, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        LinearLayout actions = new LinearLayout(service);
        actions.setOrientation(LinearLayout.HORIZONTAL);
        actions.setContentDescription("剪贴板全文预览操作");
        if (insert != null) addAction(actions, "插入", "插入剪贴板全文", insert);
        if (edit != null) addAction(actions, "修改", "修改剪贴板记录", edit);
        addAction(actions, "关闭", "关闭剪贴板全文预览", () -> { });
        panel.addView(actions, new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, service.pixels(44)));

        PopupWindow window = new PopupWindow(panel, width, height, false);
        window.setBackgroundDrawable(null);
        window.setElevation(service.pixels(8));
        window.setOutsideTouchable(true);
        window.setInputMethodMode(PopupWindow.INPUT_METHOD_NOT_NEEDED);
        window.setAnimationStyle(0);
        View.OnLayoutChangeListener layout = (view, l, t, r, b, oldL, oldT, oldR, oldB) -> {
            if (l != oldL || t != oldT || r != oldR || b != oldB) dismiss();
        };
        View.OnAttachStateChangeListener attachment = new View.OnAttachStateChangeListener() {
            @Override public void onViewAttachedToWindow(View view) { }
            @Override public void onViewDetachedFromWindow(View view) { dismiss(); }
        };
        Runnable cleanup = () -> {
            anchor.removeOnLayoutChangeListener(layout);
            keyboard.removeOnLayoutChangeListener(layout);
            anchor.removeOnAttachStateChangeListener(attachment);
            body.setText("");
            if (popup == window) popup = null;
            service.setClipboardPreviewExpanded(false);
        };
        window.setOnDismissListener(cleanup::run);
        anchor.addOnLayoutChangeListener(layout);
        keyboard.addOnLayoutChangeListener(layout);
        anchor.addOnAttachStateChangeListener(attachment);
        popup = window;
        int[] inWindow = new int[2];
        anchor.getLocationOnScreen(position);
        anchor.getLocationInWindow(inWindow);
        try {
            window.showAtLocation(anchor, Gravity.TOP | Gravity.LEFT,
                left - position[0] + inWindow[0], bottom - height - position[1] + inWindow[1]);
        } catch (WindowManager.BadTokenException ignored) {
            // 输入框恰好关闭时只撤销预览，不把窗口令牌错误带到输入法服务。
            dismiss();
            cleanup.run();
        }
    }

    private void addAction(LinearLayout row, String title, String description, Runnable action) {
        Button button = new Button(service);
        button.setText(title);
        button.setContentDescription(description);
        button.setTextColor(Color.parseColor(service.skin.keyForeground()));
        button.setBackground(DrawablePolicy.rounded(
            Color.parseColor(service.skin.keyBackground()), service.pixels(8)));
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0,
            LinearLayout.LayoutParams.MATCH_PARENT, 1);
        if (row.getChildCount() > 0) params.setMarginStart(service.pixels(8));
        row.addView(button, params);
        button.setOnClickListener(ignored -> {
            service.imeKeyFeedback.playFeedback(button);
            // 先清理浮层、正文和临时父窗口，再向编辑器或应用编辑页交付动作。
            dismiss();
            action.run();
        });
    }

    private void clearPendingLayout() {
        if (pendingAnchor != null && pendingAnchor.getViewTreeObserver().isAlive())
            pendingAnchor.getViewTreeObserver().removeOnGlobalLayoutListener(pendingLayout);
        pendingAnchor = null;
        pendingLayout = null;
    }

    void dismiss() {
        clearPendingLayout();
        if (popup != null) popup.dismiss();
        service.setClipboardPreviewExpanded(false);
    }
}
