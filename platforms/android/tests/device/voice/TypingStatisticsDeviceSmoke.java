package app.msime.android.test;

import android.app.Activity;
import android.content.Intent;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import android.view.View;
import android.view.ViewGroup;
import android.webkit.WebView;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Predicate;
import org.json.JSONObject;

/** Verify aggregate-only statistics across the Tauri app and dedicated IME process. */
public final class TypingStatisticsDeviceSmoke extends DeviceSmoke {
    private static final String PAGE_BUTTON =
        "Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '打字统计')";
    private static final String RANGE_TOTAL =
        "document.querySelector('[aria-label=\"当前范围输入字符数\"]')";
    private static final String TOGGLE =
        "document.querySelector('input[aria-label=\"记录打字统计\"]')";
    private static final String REFRESH =
        "Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '刷新统计')";
    private static final String RESET =
        "Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '清空统计')";
    private Activity settingsActivity;
    private WebView web;

    @Override protected String successDescription() {
        return "aggregate-only IME writes, React display, pause, reset and cross-process resume";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath()))
            throw new AssertionError("Statistics escaped the preview sandbox");
        File preferences = new File(directory, "preferences.json");
        File statistics = new File(directory, "typing-statistics.json");
        byte[] originalPreferences = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        byte[] originalStatistics = statistics.exists() ? Files.readAllBytes(statistics.toPath()) : null;
        long revision = originalPreferences == null ? 0
            : new JSONObject(new String(originalPreferences, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject preferenceSnapshot = new JSONObject().put("format_version", 1).put("revision", revision + 1)
            .put("preferences", new JSONObject(options.getJSONObject("preferences").toString())
                .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
                .put("last_chinese_scheme", "quanpin"));
        try {
            stage = "controlled statistics baseline";
            publish(preferences, preferenceSnapshot.toString().getBytes(StandardCharsets.UTF_8));
            Files.deleteIfExists(statistics.toPath());
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            shell("ime disable app.msime.android/app.msime.android.MSIMEInputService");
            shell("ime enable app.msime.android/app.msime.android.MSIMEInputService");
            shell("ime set app.msime.android/app.msime.android.MSIMEInputService");
            SystemClock.sleep(1000);

            stage = "IME aggregate write";
            typeSyntheticPhrase();
            JSONObject first = awaitStatistics(statistics, value -> value.optLong("total") == 2);
            assertAggregate(first, true, 2);
            String persisted = new String(Files.readAllBytes(statistics.toPath()), StandardCharsets.UTF_8);
            if (persisted.contains("nihao") || persisted.contains("你好"))
                throw new AssertionError("Statistics persisted committed content");

            stage = "React cross-process statistics read";
            openSettings();
            awaitJs("!!(" + PAGE_BUTTON + ")");
            js("(" + PAGE_BUTTON + ").click(); true");
            awaitJs("(" + RANGE_TOTAL + ")?.textContent === '2'");
            awaitJs("Array.from(document.querySelectorAll('[aria-label]')).some(node => node.getAttribute('aria-label')?.startsWith('汉字 2 字符'))");
            awaitJs("Array.from(document.querySelectorAll('[aria-label]')).some(node => node.getAttribute('aria-label')?.startsWith('全拼 26 键 2 字符'))");

            stage = "React disables statistics";
            js("if ((" + TOGGLE + ").checked) (" + TOGGLE + ").click(); true");
            awaitJs("(" + TOGGLE + ").checked === false && !(" + TOGGLE + ").disabled");
            awaitStatistics(statistics, value -> !value.optBoolean("enabled", true) && value.optLong("total") == 2);
            typeSyntheticPhrase();
            SystemClock.sleep(1200);
            if (readStatistics(statistics).optLong("total") != 2)
                throw new AssertionError("Disabled statistics changed");

            stage = "React reset confirmation";
            openSettings();
            js("window.confirm = () => false; (" + RESET + ").click(); true");
            SystemClock.sleep(300);
            if (readStatistics(statistics).optLong("total") != 2)
                throw new AssertionError("Cancelled reset changed statistics");
            js("window.confirm = () => true; (" + RESET + ").click(); true");
            awaitJs("(" + RANGE_TOTAL + ")?.textContent === '0'");
            JSONObject cleared = awaitStatistics(statistics,
                value -> !value.optBoolean("enabled", true) && value.optLong("total") == 0);
            if (cleared.getJSONObject("days").length() != 0
                    || cleared.getJSONObject("detail").getJSONObject("characters").length() != 0
                    || cleared.getJSONObject("detail").getJSONObject("sources").length() != 0)
                throw new AssertionError("Reset retained aggregate counts");

            stage = "React re-enables statistics";
            js("if (!(" + TOGGLE + ").checked) (" + TOGGLE + ").click(); true");
            awaitJs("(" + TOGGLE + ").checked === true && !(" + TOGGLE + ").disabled");
            awaitStatistics(statistics, value -> value.optBoolean("enabled", false) && value.optLong("total") == 0);

            stage = "IME resumes after app-process write";
            typeSyntheticPhrase();
            JSONObject resumed = awaitStatistics(statistics, value -> value.optLong("total") == 2);
            assertAggregate(resumed, true, 2);
            openSettings();
            js("(" + REFRESH + ").click(); true");
            awaitJs("(" + RANGE_TOTAL + ")?.textContent === '2'");
        } finally {
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            SystemClock.sleep(500);
            restore(preferences, originalPreferences);
            restore(statistics, originalStatistics);
        }
    }

    private void typeSyntheticPhrase() throws Exception {
        shell("am force-stop app.msime.android.test");
        shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
        for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
        tap(key("空格"));
        await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));
    }

    private void assertAggregate(JSONObject value, boolean enabled, long total) throws Exception {
        if (value.getBoolean("enabled") != enabled || value.getLong("total") != total
                || value.getJSONObject("days").length() != 1
                || value.getJSONObject("detail").getJSONObject("characters").optLong("han") != total
                || value.getJSONObject("detail").getJSONObject("sources").optLong("quanpin") != total)
            throw new AssertionError("Aggregate classification or source was incorrect");
    }

    private JSONObject readStatistics(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    private JSONObject awaitStatistics(File file, Predicate<JSONObject> predicate) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if (file.exists()) {
                JSONObject value = readStatistics(file);
                if (predicate.test(value)) return value;
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected aggregate statistics state was not observed");
    }

    private void openSettings() throws Exception {
        if (settingsActivity == null) {
            Intent intent = new Intent().setClassName(getTargetContext(), "app.msime.android.MainActivity")
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
            settingsActivity = startActivitySync(intent);
        } else {
            shell("am start -W -n app.msime.android/.MainActivity");
        }
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            runOnMainSync(() -> web = findWebView(settingsActivity.getWindow().getDecorView()));
            if (web != null) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Tauri WebView not created");
    }

    private WebView findWebView(View view) {
        if (view instanceof WebView) return (WebView) view;
        if (view instanceof ViewGroup) {
            ViewGroup group = (ViewGroup) view;
            for (int index = 0; index < group.getChildCount(); index++) {
                WebView found = findWebView(group.getChildAt(index));
                if (found != null) return found;
            }
        }
        return null;
    }

    private String js(String expression) throws Exception {
        for (int attempt = 0; attempt < 3; attempt++) {
            CountDownLatch completed = new CountDownLatch(1);
            AtomicReference<String> result = new AtomicReference<>();
            runOnMainSync(() -> {
                web = findWebView(settingsActivity.getWindow().getDecorView());
                if (web != null) web.evaluateJavascript(expression, value -> {
                    result.set(value);
                    completed.countDown();
                });
            });
            if (completed.await(5, TimeUnit.SECONDS)) return result.get();
            SystemClock.sleep(100);
        }
        throw new AssertionError("WebView response timed out");
    }

    private void awaitJs(String condition) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if ("true".equals(js(condition))) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected React statistics state was not observed");
    }

    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { /* Discard synthetic command output. */ }
        }
    }

    private void publish(File file, byte[] contents) throws Exception {
        AtomicFile target = new AtomicFile(file);
        FileOutputStream output = target.startWrite();
        try {
            output.write(contents);
            target.finishWrite(output);
        } catch (Exception error) {
            target.failWrite(output);
            throw error;
        }
    }

    private void restore(File file, byte[] contents) throws Exception {
        if (contents == null) Files.deleteIfExists(file.toPath());
        else publish(file, contents);
    }
}
