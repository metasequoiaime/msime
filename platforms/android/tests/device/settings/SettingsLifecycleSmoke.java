package app.msime.android.test;

import android.os.ParcelFileDescriptor;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;

/** Controller lives outside the product, so normal Tauri window exit is observable. */
public final class SettingsLifecycleSmoke extends DeviceSmoke {
    @Override protected String successDescription() { return "IME process survives settings close and reopen with working input"; }
    @Override protected void runChecks() throws Exception {
        stage = "initial IME process";
        // The preceding instrumentation may force-stop its target package. Rebind
        // once before measuring; never rebind between either settings close and input.
        shell("ime disable app.msime.android/app.msime.android.MSIMEInputService");
        shell("ime enable app.msime.android/app.msime.android.MSIMEInputService");
        shell("ime set app.msime.android/app.msime.android.MSIMEInputService");
        android.os.SystemClock.sleep(1000);
        shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
        await(key("n"));
        String originalPid = shell("pidof app.msime.android:ime").trim();
        if (!originalPid.matches("[0-9]+")) throw new AssertionError("Dedicated IME process missing");
        for (int iteration = 0; iteration < 2; iteration++) {
            stage = "settings open and close";
            shell("am start -W -n app.msime.android/.MainActivity");
            await(node -> equalsText("app.msime.android", node.getPackageName()) && equalsText("android.webkit.WebView", node.getClassName()));
            shell("input keyevent 4");
            await(field("msime-test-plain"));
            String afterPid = shell("pidof app.msime.android:ime").trim();
            if (!originalPid.equals(afterPid)) throw new AssertionError("Closing settings restarted the IME process");
            stage = "input after settings close";
            tap(field("msime-test-plain"));
            for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
            tap(key("空格"));
            String expected = iteration == 0 ? "你好" : "你好你好";
            await(field("msime-test-plain").and(node -> equalsText(expected, node.getText())));
        }
    }
    private String shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command)); var output = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[1024];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
            return new String(output.toByteArray(), StandardCharsets.UTF_8);
        }
    }
}
