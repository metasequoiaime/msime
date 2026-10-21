package app.msime.android.test;

import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** 用真实密码框验收自动数字行、跨输入框保留及系统输入法切换复位。 */
public class AutomaticNumberRowDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "password number row activates, retains across editors and resets after IME switching without changing preferences";
    }

    @Override protected void runChecks() throws Exception {
        File local = new File(getTargetContext().getFilesDir(), "bootstrap/state/android-settings.json");
        byte[] original = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        byte[] disabled = new JSONObject().put("version", 1).put("settings", new JSONObject()
            .put("platform.android.number_row", false)).toString().getBytes(StandardCharsets.UTF_8);
        String otherIme = null;
        for (String candidate : shellOutput("ime list -s").split("\\s+")) {
            if (!candidate.isEmpty() && !candidate.startsWith("app.msime.android/")) {
                otherIme = candidate;
                break;
            }
        }
        if (otherIme == null || !otherIme.matches("[A-Za-z0-9_.$/]+"))
            throw new AssertionError("A second enabled IME is required");
        try {
            stage = "manual number row disabled";
            publish(local, disabled);
            restartIme();
            openEditor();
            awaitStableBounds(key("q"));
            assertNoNumberRow();

            stage = "password activates a number row above letters";
            tap(field("msime-test-password"));
            AccessibilityNodeInfo one = awaitStableBounds(key("1"));
            android.graphics.Rect digit = new android.graphics.Rect();
            android.graphics.Rect letter = new android.graphics.Rect();
            one.getBoundsInScreen(digit);
            awaitStableBounds(key("q")).getBoundsInScreen(letter);
            if (digit.bottom > letter.top) throw new AssertionError("Digit is not above letter row");
            tap(key("5"));
            // 密码辅助功能节点可能只暴露掩码：检查实际字数，不读取或输出内容。
            await(field("msime-test-password").and(node -> node.getText() != null && node.getText().length() == 1));

            stage = "ordinary editor retains automatic row";
            tap(field("msime-test-plain"));
            awaitStableBounds(key("1"));
            SystemClock.sleep(1000);
            tap(key("5"));
            await(field("msime-test-plain").and(node -> equalsText("5", node.getText())));

            stage = "switch away and back clears automatic row";
            shellDiscard("ime set " + otherIme);
            awaitSelectedIme(otherIme);
            shellDiscard("ime set app.msime.android/.MSIMEInputService");
            awaitSelectedIme("app.msime.android/.MSIMEInputService");
            awaitStableBounds(key("q"));
            assertNoNumberRow();

            stage = "switch back in a password reactivates automatic row";
            tap(field("msime-test-password"));
            awaitStableBounds(key("1"));
            shellDiscard("ime set " + otherIme);
            awaitSelectedIme(otherIme);
            shellDiscard("ime set app.msime.android/.MSIMEInputService");
            awaitSelectedIme("app.msime.android/.MSIMEInputService");
            awaitStableBounds(key("1"));
            if (!java.util.Arrays.equals(disabled, Files.readAllBytes(local.toPath())))
                throw new AssertionError("Automatic mode modified the saved preference");
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(local.toPath());
            else publish(local, original);
            restartIme();
        }
    }

    private void assertNoNumberRow() {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            if (window.getType() == AccessibilityWindowInfo.TYPE_INPUT_METHOD
                    && find(window.getRoot(), key("1")) != null)
                throw new AssertionError("Automatic number row did not reset");
        }
    }

    private void awaitSelectedIme(String ime) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 10000;
        while (SystemClock.uptimeMillis() < deadline) {
            if (ime.equals(shellOutput("settings get secure default_input_method"))) {
                SystemClock.sleep(500);
                return;
            }
            SystemClock.sleep(100);
        }
        throw new AssertionError("System IME switch did not settle");
    }

    private String shellOutput(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            byte[] buffer = new byte[1024];
            int length;
            while ((length = input.read(buffer)) != -1) output.write(buffer, 0, length);
            return new String(output.toByteArray(), StandardCharsets.UTF_8).trim();
        }
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
    }
}
