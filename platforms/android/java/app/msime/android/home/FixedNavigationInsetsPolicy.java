package app.msime.android.home;

import android.view.View;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowInsetsCompat;
import app.msime.android.WindowInsetsPolicy;

/** 详情页固定分类导航与输入法窗口状态的接线。 */
final class FixedNavigationInsetsPolicy {
    private FixedNavigationInsetsPolicy() {}

    static void bind(View root, View navigation, Runnable reposition) {
        ViewCompat.setOnApplyWindowInsetsListener(root, (target, insets) -> {
            navigation.setVisibility(WindowInsetsPolicy.shouldShowFixedPageNavigation(
                insets.isVisible(WindowInsetsCompat.Type.ime())) ? View.VISIBLE : View.GONE);
            reposition.run();
            return insets;
        });
        ViewCompat.requestApplyInsets(root);
    }
}
