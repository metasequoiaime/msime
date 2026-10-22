package app.msime.android.test;

import android.accessibilityservice.AccessibilityService;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.Build;
import android.os.SystemClock;
import android.provider.Settings;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import android.view.autofill.AutofillManager;
import java.util.function.BooleanSupplier;
import java.util.Objects;
import java.io.File;
import java.io.FileOutputStream;

/** 独立编辑器 UID 经真实系统服务、渲染器和点击验收，结束恢复原填充服务。 */
public final class InlineAutofillDeviceSmoke extends DeviceSmoke {
    private InlineAutofillEditorActivity editor;

    @Override protected String successDescription() {
        return "system autofill request, " + (Build.VERSION.SDK_INT >= 30 ? "inline rendering and lifecycle" : "legacy menu")
            + ", multi-field filling and service restoration";
    }

    @Override protected void runChecks() throws Exception {
        String previous = Settings.Secure.getString(getContext().getContentResolver(), "autofill_service");
        if (previous != null && !previous.matches("[A-Za-z0-9_./]*")) throw new AssertionError("Unexpected fixture service setting");
        try {
            stage = "service restoration states";
            restoreAutofillService(null);
            restoreAutofillService("");
            restoreAutofillService("app.msime.android.test/.SyntheticAutofillService");
            restartIme();
            stage = "system fill request";
            SyntheticAutofillService.empty = false;
            SyntheticAutofillService.requests = 0;
            SyntheticAutofillService.inlineRequests = 0;
            openEditor();
            tap(field("autofill-plain"));
            await(key("⌫"));
            tap(field("autofill-username"));
            await(field("autofill-username").and(AccessibilityNodeInfo::isFocused));
            boolean[] enabled = {false};
            runOnMainSync(() -> enabled[0] = editor.getSystemService(AutofillManager.class).isEnabled());
            if (!enabled[0]) throw new AssertionError("Synthetic system autofill service is not enabled");
            awaitCondition(() -> SyntheticAutofillService.requests > 0);
            if (Build.VERSION.SDK_INT >= 30) {
                stage = "inline system specifications";
                awaitCondition(() -> SyntheticAutofillService.inlineRequests > 0);
                stage = "inline remote view in keyboard row";
                awaitInline();
                stage = "keyboard hide and show";
                automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_BACK);
                SystemClock.sleep(500);
                assertNoInline();
                tap(field("autofill-username"));
                runOnMainSync(() -> editor.getSystemService(AutofillManager.class).requestAutofill(editor.username));
                awaitInline();
                stage = "switch to non-autofill field";
                tap(field("autofill-plain"));
                SystemClock.sleep(800);
                assertNoInline();
                tap(field("autofill-password"));
                runOnMainSync(() -> editor.getSystemService(AutofillManager.class).requestAutofill(editor.password));
                awaitInline();
            }
            stage = "system multi-field fill";
            tap(node -> equalsText(SyntheticAutofillService.LABEL, node.getText()));
            awaitCondition(() -> {
                boolean[] filled = {false};
                runOnMainSync(() -> filled[0] = SyntheticAutofillService.USERNAME.contentEquals(editor.username.getText())
                    && SyntheticAutofillService.PASSWORD.contentEquals(editor.password.getText()));
                return filled[0];
            });
            if (Build.VERSION.SDK_INT >= 30) {
                stage = "empty response restores toolbar";
                SyntheticAutofillService.empty = true;
                int previousRequests = SyntheticAutofillService.requests;
                openEditor();
                tap(field("autofill-plain"));
                await(key("⌫"));
                tap(field("autofill-username"));
                awaitCondition(() -> SyntheticAutofillService.requests > previousRequests);
                SystemClock.sleep(1000);
                assertNoInline();
                await(key("⌫"));
            }
        } finally {
            if (editor != null) runOnMainSync(() -> editor.finish());
            SyntheticAutofillService.empty = false;
            restoreAutofillService(previous);
        }
    }

    private void restoreAutofillService(String value) throws Exception {
        // executeShellCommand 不解析 shell 引号；空字符串用提供方接口传递，不能写成两个引号。
        shellDiscard(value == null ? "settings delete secure autofill_service"
            : value.isEmpty() ? "content call --uri content://settings/secure --method PUT_secure"
                + " --arg autofill_service --extra value:s:"
            : "settings put secure autofill_service " + value);
        String restored = Settings.Secure.getString(getContext().getContentResolver(), "autofill_service");
        if (!Objects.equals(value, restored)) throw new AssertionError("Autofill service setting was not restored exactly");
    }

    private void openEditor() {
        if (editor != null) runOnMainSync(() -> editor.finish());
        Intent intent = new Intent(getContext(), InlineAutofillEditorActivity.class)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        editor = (InlineAutofillEditorActivity) startActivitySync(intent);
    }

    private void awaitInline() throws Exception {
        AccessibilityNodeInfo row = await(described("自动填充"));
        SystemClock.sleep(500);
        saveInlineScreenshot();
        AccessibilityNodeInfo label = await(node -> equalsText(SyntheticAutofillService.LABEL, node.getText()));
        Rect rowBounds = new Rect();
        Rect labelBounds = new Rect();
        row.getBoundsInScreen(rowBounds);
        label.getBoundsInScreen(labelBounds);
        if (!rowBounds.contains(labelBounds)) throw new AssertionError("Fill presentation is outside the IME row");
    }

    private void assertNoInline() {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            if (find(window.getRoot(), described("自动填充")) != null)
                throw new AssertionError("Stale inline row remained visible");
        }
    }

    private void saveInlineScreenshot() throws Exception {
        Rect bounds = new Rect();
        for (AccessibilityWindowInfo window : automation.getWindows())
            if (window.getType() == AccessibilityWindowInfo.TYPE_INPUT_METHOD) window.getBoundsInScreen(bounds);
        Bitmap screen = automation.takeScreenshot();
        if (screen == null) throw new AssertionError("Synthetic IME screenshot unavailable");
        try {
            if (!bounds.intersect(0, 0, screen.getWidth(), screen.getHeight()))
                throw new AssertionError("Synthetic IME screenshot bounds unavailable");
            Bitmap keyboard = Bitmap.createBitmap(screen, bounds.left, bounds.top, bounds.width(), bounds.height());
            try (var output = new FileOutputStream(new File(getContext().getExternalFilesDir(null), "inline-autofill.png"))) {
                if (!keyboard.compress(Bitmap.CompressFormat.PNG, 100, output))
                    throw new AssertionError("Synthetic IME screenshot encoding failed");
            } finally { if (keyboard != screen) keyboard.recycle(); }
        } finally { screen.recycle(); }
    }

    private static void awaitCondition(BooleanSupplier condition) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if (condition.getAsBoolean()) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic autofill system event did not arrive");
    }
}
