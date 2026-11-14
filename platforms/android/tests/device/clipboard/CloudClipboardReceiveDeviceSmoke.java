package app.msime.android.test;

import android.app.Instrumentation;
import android.content.ClipData;
import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import android.os.PersistableBundle;
import java.lang.reflect.Method;
import java.time.Instant;
import java.util.List;

/** 用合成云端页面走产品接收方法，核对 Android 系统剪贴板；不连接真实账号。 */
public final class CloudClipboardReceiveDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            ClipboardManager clipboard = getTargetContext().getSystemService(ClipboardManager.class);
            ClipData original = clipboard.getPrimaryClip();
            try {
                ClassLoader loader = getTargetContext().getClassLoader();
                Class<?> serviceType = loader.loadClass("app.msime.android.MSIMEInputService");
                Object service = serviceType.getConstructor().newInstance();
                Method attach = android.content.ContextWrapper.class.getDeclaredMethod("attachBaseContext", Context.class);
                attach.setAccessible(true);
                attach.invoke(service, getTargetContext());
                Class<?> receiverType = loader.loadClass("app.msime.android.ImeCloudClipboardAutoReceive");
                var constructor = receiverType.getDeclaredConstructor(serviceType);
                constructor.setAccessible(true);
                Object receiver = constructor.newInstance(service);
                Class<?> policyType = loader.loadClass("app.msime.android.clipboard.CloudClipboardReceivePolicy");
                Class<?> pageType = loader.loadClass("app.msime.android.BackendAccount$ClipboardPage");
                Class<?> itemType = loader.loadClass("app.msime.android.BackendAccount$ClipboardItem");
                Method apply = receiverType.getDeclaredMethod("apply", policyType, long.class, pageType);
                apply.setAccessible(true);
                var pageConstructor = pageType.getConstructor(boolean.class, List.class, String.class, String.class);
                var itemConstructor = itemType.getConstructor(String.class, String.class, String.class);
                Object item = itemConstructor.newInstance("synthetic", "Synthetic remote clipboard",
                    Instant.now().plusSeconds(60).toString());
                Object page = pageConstructor.newInstance(true, List.of(item), "ready", "");
                Object policy = policyType.getConstructor().newInstance();
                clipboard.clearPrimaryClip();
                apply.invoke(receiver, policy, 0L, page);
                check("Synthetic remote clipboard".contentEquals(clipboard.getPrimaryClip().getItemAt(0).getText()),
                    "远端新内容没有写入系统剪贴板");
                check("水杉云剪贴板".contentEquals(clipboard.getPrimaryClipDescription().getLabel()),
                    "缺少跨服务重启保留的接收标记");
                Method read = serviceType.getDeclaredMethod("readPrimaryClip");
                read.setAccessible(true);
                Method changed = receiverType.getDeclaredMethod("changed", read.getReturnType());
                changed.setAccessible(true);
                changed.invoke(receiver, read.invoke(service));

                for (String blocked : List.of("cancelled", "newer-local", "sensitive", "image", "disabled", "pending")) {
                    policy = policyType.getConstructor().newInstance();
                    clipboard.setPrimaryClip(ClipData.newPlainText("Synthetic local", "Synthetic local clipboard"));
                    Object candidate = page;
                    if (blocked.equals("cancelled")) policyType.getMethod("invalidate").invoke(policy);
                    if (blocked.equals("newer-local")) {
                        Object old = itemConstructor.newInstance("synthetic-old", "Synthetic older remote", "2000-01-01T00:00:00Z");
                        candidate = pageConstructor.newInstance(true, List.of(old), "ready", "");
                    }
                    if (blocked.equals("sensitive")) {
                        ClipData sensitive = ClipData.newPlainText("Synthetic sensitive", "Synthetic local clipboard");
                        PersistableBundle extras = new PersistableBundle();
                        extras.putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true);
                        sensitive.getDescription().setExtras(extras);
                        clipboard.setPrimaryClip(sensitive);
                    }
                    if (blocked.equals("image")) clipboard.setPrimaryClip(new ClipData(
                        new ClipDescription("Synthetic image", new String[] {"image/png"}),
                        new ClipData.Item(Uri.parse("content://app.msime.android.test/synthetic-image"))));
                    if (blocked.equals("disabled")) candidate = pageConstructor.newInstance(false, List.of(item), "disabled", "");
                    if (blocked.equals("pending")) candidate = pageConstructor.newInstance(true, List.of(item), "pending", "synthetic");
                    ClipData before = clipboard.getPrimaryClip();
                    apply.invoke(receiver, policy, 0L, candidate);
                    ClipData after = clipboard.getPrimaryClip();
                    check(before.getDescription().getLabel().equals(after.getDescription().getLabel())
                            && java.util.Objects.equals(before.getItemAt(0).getText(), after.getItemAt(0).getText())
                            && java.util.Objects.equals(before.getItemAt(0).getUri(), after.getItemAt(0).getUri()),
                        "接收覆盖了需要保留的系统内容：" + blocked);
                }
            } catch (Exception | AssertionError error) {
                failure[0] = error instanceof java.lang.reflect.InvocationTargetException && error.getCause() != null
                    ? error.getCause() : error;
            }
            finally {
                if (original == null) clipboard.clearPrimaryClip(); else clipboard.setPrimaryClip(original);
            }
        });
        Bundle result = new Bundle();
        result.putString("stream", failure[0] == null
            ? "MSIME_DEVICE_SMOKE_PASSED: cloud receive writes system clipboard, marks echoes and preserves cancelled, newer, sensitive and non-text content\n"
            : "MSIME_DEVICE_SMOKE_FAILED: synthetic cloud receive (" + failure[0].getClass().getSimpleName() + ") " + failure[0].getMessage() + "\n");
        finish(failure[0] == null ? -1 : 0, result);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
