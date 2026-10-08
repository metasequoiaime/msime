package app.msime.android.test;

import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** Device acceptance for the quanpin nine-key expanded panel: spelling column, locks, backspace and filters inside the open panel. */
public final class NineKeyPanelDeviceSmoke extends DeviceSmoke {
    private static final String PANEL = "九键候选面板";

    @Override protected String successDescription() {
        return "nine-key expanded panel keeps open across spelling locks, backspace and filters";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()),
            StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin")
            .put("touch_keyboard_layout", "nine_key")
            .put("traditional_chinese_output", false)
            .put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", false)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("nine_key"))
                .put("selected", "nine_key"));
        try {
            stage = "enable shared nine-key preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));

            // 6464224：6464 可以是 ming 或 ning，224 可以是 bai 或 cai。
            stage = "nine-key composition";
            for (String face : new String[] {"MNO", "GHI", "MNO", "GHI", "ABC", "ABC", "GHI"})
                tap(key(face));
            stage = "collapsed spelling column";
            await(described("选择拼音 ning"));

            stage = "open the three-column panel";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            await(described(PANEL));
            await(inPanel(described("候选面板 重输")));
            await(inPanel(described("只显示单字")));

            stage = "lock ning inside the panel";
            tap(inPanel(described("选择拼音 ning")));
            stage = "lock bai inside the panel";
            tap(inPanel(described("选择拼音 bai")));
            stage = "panel stays open after locking every digit";
            await(described(PANEL));
            await(inPanel(described("选择拼音 cai")));

            // 全部锁定时 ⌫ 撤销最后一次锁定（回到 ning'224，拼音栏仍是 224 的选项），面板不收起；再按一次才删数字（ning'22，没有 bai 了）。
            stage = "backspace undoes the last lock and keeps the panel";
            tap(described("候选面板 删除"));
            await(described(PANEL));
            await(inPanel(described("选择拼音 bai")));
            stage = "second backspace deletes a digit and keeps the panel";
            tap(described("候选面板 删除"));
            await(described(PANEL));
            await(inPanel(described("选择拼音 ba")));
            // 删掉的是数字：读音从 ning'224 变成 ning'22（显示为 ning'ba）。拼音栏现在也列出以这两个数字开头的更长音节（ba、bai、ban…），所以不能再用「bai 消失」来判断。
            await(imeTextContains("ning'ba"));

            stage = "single-character filter";
            tap(inPanel(described("只显示单字")));
            await(inPanel(described("显示全部候选")).and(AccessibilityNodeInfo::isSelected));
            AccessibilityNodeInfo first = await(inPanel(describedPrefix("候选 1：")));
            String text = first.getText() == null ? "" : first.getText().toString().split("\n")[0];
            if (text.codePointCount(0, text.length()) != 1)
                throw new AssertionError("Single-character filter left a longer candidate");
            tap(inPanel(described("显示全部候选")));
            await(inPanel(described("只显示单字")));

            stage = "stroke column toggle";
            tap(inPanel(described("切换到笔画筛选")));
            await(inPanel(described("笔画筛选 横")));
            await(inPanel(described("未选笔画")));
            tap(inPanel(described("切换到拼音")));
            await(inPanel(described("选择拼音 ba")));

            stage = "return collapses the panel";
            tap(inPanel(described("收起候选面板")));
            awaitGone(described(PANEL));
            await(described("选择拼音 ba"));

            stage = "cancel inside the panel ends the composition and closes it";
            tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
            await(described(PANEL));
            tap(inPanel(described("候选面板 重输")));
            awaitGone(described(PANEL));
            awaitGone(describedPrefix("选择拼音 "));
            // 九键组字不在输入框里标记，重输后输入框应当仍是空的。
            AccessibilityNodeInfo editor = await(field("msime-test-plain"));
            if (editor.getText() != null && editor.getText().length() > 0 && !editor.isShowingHintText())
                throw new AssertionError("Cancelled nine-key composition reached the editor");
        } finally {
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    /** 只认三栏面板里的控件：收起时的侧栏拼音列在面板下面仍在树里，描述相同。 */
    private Predicate<AccessibilityNodeInfo> inPanel(Predicate<AccessibilityNodeInfo> match) {
        return match.and(node -> {
            for (AccessibilityNodeInfo parent = node.getParent(); parent != null;
                    parent = parent.getParent()) {
                if (equalsText(PANEL, parent.getContentDescription())) return true;
            }
            return false;
        });
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            boolean present = false;
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                if (find(window.getRoot(), match) != null) {
                    present = true;
                    break;
                }
            }
            if (!present) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic UI state did not go away; IME showed " + imeTexts());
    }

    private void restartIme() throws Exception {
        shell("ime disable app.msime.android/.MSIMEInputService");
        shell("ime enable app.msime.android/.MSIMEInputService");
        shell("ime set app.msime.android/.MSIMEInputService");
        SystemClock.sleep(1000);
    }

    private void openEditor() throws Exception {
        shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }

    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
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
}
