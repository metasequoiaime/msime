package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.lang.reflect.Method;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.Predicate;

/** 阻塞真实共享存储队列，在保存完成之前重建编辑器视图，检查忙碌状态和成功交付。 */
public class ClipboardEditLifecycleDeviceSmoke extends DeviceSmoke {
    private Class<?> fragmentClass;
    private Class<?> managerClass;
    private Method hostRun;
    private String scenario;

    @Override public void onCreate(Bundle args) {
        scenario = args == null ? "detach" : args.getString("scenario", "detach");
        super.onCreate(args);
    }

    @Override protected String successDescription() {
        return "clipboard save remains pending across view recreation and delivers its result once";
    }

    @Override protected void runChecks() throws Exception {
        fragmentClass = Class.forName("androidx.fragment.app.Fragment");
        managerClass = Class.forName("androidx.fragment.app.FragmentManager");
        hostRun = Class.forName("app.msime.android.home.HostTask").getMethod(
            "run", fragmentClass, Function.class, Consumer.class);
        String key = (String) Class.forName("app.msime.android.ClipboardEditKey").getMethod(
            "editKey", Context.class, long.class, String.class).invoke(null, getTargetContext(),
                1700000000002L, ClipboardEditDeviceSmoke.ORIGINAL);
        Bundle args = new Bundle();
        args.putString("entry", key);
        Intent intent = new Intent().setClassName(getTargetContext(), "app.msime.android.home.HomeActivity")
            .putExtra("msime_open_page", "CLIPBOARD_EDIT").putExtra("msime_page_args", args)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        AtomicReference<Activity> activity = new AtomicReference<>(startActivitySync(intent));
        CountDownLatch entered = new CountDownLatch(1);
        CountDownLatch release = new CountDownLatch(1);
        try {
            stage = "clipboard editor ready";
            await(described("剪贴板记录的文字").and(node -> equalsText(ClipboardEditDeviceSmoke.ORIGINAL, node.getText())));
            Object page = main(() -> findPage(activity.get().getClass().getMethod("getSupportFragmentManager").invoke(activity.get())));
            if (page == null) throw new AssertionError("Synthetic clipboard page not found");
            main(() -> {
                hostRun.invoke(null, page, (Function<Context, Boolean>) context -> {
                    entered.countDown();
                    try {
                        if (!release.await(120, TimeUnit.SECONDS)) throw new AssertionError("Synthetic save gate timed out");
                    } catch (InterruptedException error) {
                        Thread.currentThread().interrupt();
                        throw new IllegalStateException("Synthetic save gate interrupted", error);
                    }
                    return Boolean.TRUE;
                }, (Consumer<Boolean>) ignored -> { });
                return null;
            });
            if (!entered.await(15, TimeUnit.SECONDS)) throw new AssertionError("Shared queue gate did not start");
            stage = "clipboard save submitted";
            Bundle replacement = new Bundle();
            replacement.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,
                ClipboardEditDeviceSmoke.REPLACEMENT);
            if (!await(described("剪贴板记录的文字")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, replacement))
                throw new AssertionError("Synthetic clipboard edit action refused");
            if (!await(hostText("保存").and(AccessibilityNodeInfo::isEnabled)).performAction(AccessibilityNodeInfo.ACTION_CLICK))
                throw new AssertionError("Synthetic clipboard save action refused");
            await(hostText("正在保存…").and(node -> !node.isEnabled()));
            stage = "clipboard view recreated while save is pending";
            if (scenario.equals("activity")) {
                ActivityMonitor monitor = addMonitor("app.msime.android.home.HomeActivity", null, false);
                try {
                    runOnMainSync(activity.get()::recreate);
                    Activity recreated = waitForMonitorWithTimeout(monitor, 15000);
                    if (recreated == null) throw new AssertionError("Host activity was not recreated");
                    activity.set(recreated);
                } finally {
                    removeMonitor(monitor);
                }
            } else {
                main(() -> {
                    Object manager = fragmentClass.getMethod("getParentFragmentManager").invoke(page);
                    transact(manager, page, "detach");
                    transact(manager, page, "attach");
                    return null;
                });
            }
            await(described("剪贴板记录的文字").and(node -> equalsText(ClipboardEditDeviceSmoke.REPLACEMENT, node.getText())));
            AccessibilityNodeInfo pendingButton = await(hostText("正在保存…").and(node -> !node.isEnabled()));
            Object currentPage = main(() -> findPage(activity.get().getClass().getMethod("getSupportFragmentManager").invoke(activity.get())));
            if (currentPage == null) throw new AssertionError("Recreated clipboard page not found");
            if (scenario.equals("failure")) {
                stage = "clipboard save failure reaches recreated view";
                File history = new File(getTargetContext().getFilesDir(), "MSIME/clipboard_history.json");
                byte[] original = Files.readAllBytes(history.toPath());
                publish(history, "[synthetic-invalid-history".getBytes(StandardCharsets.UTF_8));
                try {
                    release.countDown();
                    awaitSaveEnabled(pendingButton);
                    stage = "clipboard failed save retains draft";
                    await(described("剪贴板记录的文字").and(node -> equalsText(ClipboardEditDeviceSmoke.REPLACEMENT, node.getText())));
                } finally {
                    publish(history, original);
                }
                stage = "clipboard save retries after failure";
                if (!awaitSaveEnabled(pendingButton).performAction(AccessibilityNodeInfo.ACTION_CLICK))
                    throw new AssertionError("Synthetic clipboard retry action refused");
            }
            stage = "clipboard save result reaches recreated view";
            release.countDown();
            long deadline = SystemClock.uptimeMillis() + 15000;
            while (Boolean.TRUE.equals(main(() -> fragmentClass.getMethod("isAdded").invoke(currentPage)))
                    && !activity.get().isFinishing()) {
                if (SystemClock.uptimeMillis() >= deadline) throw new AssertionError("Successful save did not close the recreated editor");
                SystemClock.sleep(100);
            }
        } finally {
            release.countDown();
            runOnMainSync(activity.get()::finish);
        }
    }

    /** Android 9 的 Toast 窗口可能留下旧节点快照；刷新同一个保存按钮，按真实状态等结果。 */
    private AccessibilityNodeInfo awaitSaveEnabled(AccessibilityNodeInfo button) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if (button.refresh() && hostText("保存").test(button) && button.isEnabled()) return button;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic save button did not recover after failure");
    }

    private void transact(Object manager, Object page, String action) throws Exception {
        Class<?> transactionClass = Class.forName("androidx.fragment.app.FragmentTransaction");
        Object transaction = managerClass.getMethod("beginTransaction").invoke(manager);
        transactionClass.getMethod(action, fragmentClass).invoke(transaction, page);
        transactionClass.getMethod("commitNow").invoke(transaction);
    }

    private Object findPage(Object manager) throws Exception {
        List<?> pages = (List<?>) managerClass.getMethod("getFragments").invoke(manager);
        for (Object page : pages) {
            if (page.getClass().getName().equals("app.msime.android.home.ClipboardEditPage")) return page;
            Object found = findPage(fragmentClass.getMethod("getChildFragmentManager").invoke(page));
            if (found != null) return found;
        }
        return null;
    }

    @FunctionalInterface private interface MainCall { Object run() throws Exception; }

    private Object main(MainCall call) throws Exception {
        AtomicReference<Object> result = new AtomicReference<>();
        AtomicReference<Exception> failure = new AtomicReference<>();
        runOnMainSync(() -> {
            try { result.set(call.run()); } catch (Exception error) { failure.set(error); }
        });
        if (failure.get() != null) throw failure.get();
        return result.get();
    }

    private Predicate<AccessibilityNodeInfo> hostText(String text) {
        return node -> equalsText(getTargetContext().getPackageName(), node.getPackageName())
            && equalsText(text, node.getText());
    }

    @Override protected Predicate<AccessibilityNodeInfo> described(String text) {
        return node -> equalsText(getTargetContext().getPackageName(), node.getPackageName())
            && equalsText(text, node.getContentDescription());
    }

    @Override protected String imeTexts() { return "[omitted]"; }
}
