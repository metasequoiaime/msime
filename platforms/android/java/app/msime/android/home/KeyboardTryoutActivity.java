package app.msime.android.home;

import app.msime.android.ScreenCaptureProtection;
import android.os.Bundle;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.view.WindowCompat;

/**
 * 单独承载 AI 对话（试用键盘）的 Activity，页面本身是 {@link ChatFragment}。
 *
 * <p>宿主里的对话在 {@link HomeActivity} 底部的「对话」tab 上，宿主自己不再打开这个 Activity。它留给 Tauri 共享设置的「屏幕键盘」页：`AccountPlugin.openKeyboardTryout` 按类名启动它，那一侧没有底部 tab 可切。清单里的 `adjustResize|stateVisible` 照旧，页面一打开就弹键盘，输入框按键盘的 inset 让位。
 */
public final class KeyboardTryoutActivity extends AppCompatActivity {

    @Override protected void onCreate(@Nullable Bundle state) {
        AppMode.restore(this);
        super.onCreate(state);
        // 「防止截屏」（#10098）：窗口显示前按本地设置加上 FLAG_SECURE。
        ScreenCaptureProtection.apply(this);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        // 重建时 FragmentManager 自己恢复页面，只在第一次创建时加。
        if (state == null) {
            getSupportFragmentManager().beginTransaction()
                .add(android.R.id.content, ChatFragment.standalone())
                .commit();
        }
    }
}
