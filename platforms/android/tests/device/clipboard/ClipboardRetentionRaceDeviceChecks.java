package app.msime.android.test;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONObject;

/** 在实际产品服务方法中重放迟到关闭；不启动输入、不访问系统剪贴板。 */
final class ClipboardRetentionRaceDeviceChecks {
    private ClipboardRetentionRaceDeviceChecks() {}

    static void deliverOldDisable(DeviceSmoke owner, ClassLoader loader, Object history, Object mode) {
        AtomicReference<Throwable> failure = new AtomicReference<>();
        owner.runOnMainSync(() -> {
            try {
                Class<?> serviceClass = loader.loadClass("app.msime.android.MSIMEInputService");
                Object service = serviceClass.getConstructor().newInstance();
                Field storeField = serviceClass.getDeclaredField("clipboardHistory");
                storeField.setAccessible(true);
                storeField.set(service, history);
                Field modeField = serviceClass.getDeclaredField("clipboardMode");
                modeField.setAccessible(true);
                modeField.set(service, mode);
                Class<?> sourceClass = loader.loadClass("app.msime.android.ClipboardHistoryRetentionPolicy$Source");
                Object live = java.util.Arrays.stream(sourceClass.getEnumConstants())
                    .filter(value -> value.toString().equals("LIVE")).findFirst().orElseThrow();
                Method apply = serviceClass.getDeclaredMethod("applyClipboardPreference", JSONObject.class, sourceClass);
                apply.setAccessible(true);
                apply.invoke(service, new JSONObject().put("clipboard_history", false), live);
            } catch (ReflectiveOperationException | org.json.JSONException error) { failure.set(error); }
        });
        if (failure.get() != null) throw new AssertionError("Stale preference fixture failed", failure.get());
    }
}
