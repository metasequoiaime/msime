package app.msime.android.home;

import android.app.Activity;
import android.content.Context;
import android.text.Editable;
import android.widget.EditText;
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

    static void insert(Fragment fragment, EditText target) {
        load(fragment, entries -> {
            Context context = fragment.getContext();
            if (context == null || !target.isAttachedToWindow()) return;
            String value = text(context, entries);
            Editable content = target.getText();
            int start = Math.max(0, Math.min(target.getSelectionStart(), content.length()));
            int end = Math.max(0, Math.min(target.getSelectionEnd(), content.length()));
            if (start > end) {
                int swap = start;
                start = end;
                end = swap;
            }
            content.replace(start, end, value);
            target.setSelection(start + value.length());
        });
    }
}
