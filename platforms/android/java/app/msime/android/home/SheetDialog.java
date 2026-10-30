package app.msime.android.home;

import android.content.Context;
import android.widget.FrameLayout;
import app.msime.android.ScreenCaptureProtection;
import com.google.android.material.bottomsheet.BottomSheetBehavior;
import com.google.android.material.bottomsheet.BottomSheetDialog;

/**
 * 设置应用里每个底部面板的窗口都从这里建：按「防止截屏」加好窗口标志，打开即展开，并且跳过折叠高度。
 *
 * <p>Material 的默认是先停在折叠高度（屏宽 16:9 之外的那一截），内容比它高时，面板末尾的「保存」「重置」这类操作行会落在导航栏下面，要用户先往上拖才看得见。打开即展开后，内容放得下时面板就是内容的高度，放不下时撑到屏幕高度、里面的滚动区滚动；跳过折叠高度让往下拖就是关掉，不会停在一半把操作行又藏回屏幕下面。内容少的面板本来就比折叠高度矮，这两项对它们不改变任何东西。
 */
final class SheetDialog {
    private SheetDialog() {}

    static BottomSheetDialog create(Context context) {
        BottomSheetDialog dialog = ScreenCaptureProtection.protect(new BottomSheetDialog(context));
        BottomSheetBehavior<FrameLayout> behavior = dialog.getBehavior();
        behavior.setSkipCollapsed(true);
        behavior.setState(BottomSheetBehavior.STATE_EXPANDED);
        return dialog;
    }
}
