package app.msime.android.home;

import android.app.Activity;
import android.os.Bundle;
import app.msime.android.HostDeepLink;

/**
 * 旧入口的跳板：云剪贴板页已经搬进宿主的详情页 {@link PageId#CLOUD_CLIPBOARD}，这个 Activity 只把还指向它的 Intent（旧的快捷方式、别的模块里的跳转）转发到 HomeActivity 的深链，然后立刻结束。
 *
 * <p>类名和清单项保留，清单里用不显示界面的透明主题，所以跳转时不会闪出一个空窗口。
 */
public final class CloudClipboardActivity extends Activity {
    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        startActivity(HostDeepLink.page(this, PageId.CLOUD_CLIPBOARD.name(), null));
        finish();
    }
}
