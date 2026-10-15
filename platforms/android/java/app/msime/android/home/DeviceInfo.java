package app.msime.android.home;

import android.app.Activity;
import android.content.Context;
import androidx.fragment.app.Fragment;
import app.msime.android.DeviceInfoCollector;
import app.msime.android.DeviceInfoReport.Entry;
import java.util.List;
import java.util.function.Consumer;

/** 设置页的异步加载与复制；系统信息白名单与键盘菜单共用。 */
final class DeviceInfo {
    private DeviceInfo() {}

    static void load(Fragment fragment, Consumer<List<Entry>> done) {
        Activity activity = fragment.getActivity();
        if (activity == null) return;
        DeviceInfoCollector.Screen screen = DeviceInfoCollector.Screen.of(activity);
        HostTask.run(fragment, context -> DeviceInfoCollector.collect(context, screen), entries -> {
            if (entries != null) done.accept(entries);
        });
    }

    static String text(Context context, List<Entry> entries) {
        return DeviceInfoCollector.text(context, entries);
    }

    static void copy(Fragment fragment) {
        load(fragment, entries -> {
            Context context = fragment.getContext();
            if (context != null) ClipboardActions.copyText(context, "设备信息", text(context, entries), "已复制设备信息");
        });
    }
}
