package app.msime.android.test;

import android.app.Activity;
import android.content.Intent;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.webkit.WebView;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONArray;
import org.json.JSONObject;

/** Drive the actual React DOM and Tauri IPC, then observe a separate system editor. */
public final class SettingsDeviceSmoke extends DeviceSmoke {
    private static final String PUNCTUATION_CHECKBOX =
        "Array.from(document.querySelectorAll('label')).find(label => "
        + "label.textContent?.includes('中文标点'))?.querySelector('input[type=checkbox]')";
    private static final String RELOAD_BUTTON =
        "Array.from(document.querySelectorAll('button')).find(button => "
        + "button.textContent?.trim() === '重新读取')";
    private static final String QUANPIN_TOGGLE =
        "document.querySelector('[aria-label=\"显示输入方案 全拼 26 键\"]')";
    private static final String NINE_KEY_TOGGLE =
        "document.querySelector('[aria-label=\"显示输入方案 全拼 9 键\"]')";
    private static final String QUANPIN_SELECT =
        "document.querySelector('[aria-label=\"设为当前输入方案 全拼 26 键\"]')";
    private static final String NINE_KEY_SELECT =
        "document.querySelector('[aria-label=\"设为当前输入方案 全拼 9 键\"]')";
    private static final String WUBI_MIXED_PINYIN =
        "document.querySelector('[aria-label=\"编码打不出时用拼音候选\"]')";
    private static final String KEYBOARD_HEIGHT =
        "document.querySelector('[aria-label=\"键盘高度\"]')";
    private static final String KEY_SPACING =
        "document.querySelector('[aria-label=\"按键间距\"]')";
    private static final String ROW_SPACING =
        "document.querySelector('[aria-label=\"行间距\"]')";
    private static final String VOICE_SHORTCUT =
        "document.querySelector('[aria-label=\"顶部语音入口\"]')";
    private static final String RESET_KEYBOARD_SETTINGS =
        "document.querySelector('[aria-label=\"恢复屏幕键盘默认设置\"]')";
    private static final String MY_SKIN =
        "document.querySelector('[aria-label=\"屏幕键盘皮肤 我的皮肤\"]')";
    private static final String CUSTOM_SKIN_DESIGN_TAB =
        "Array.from(document.querySelectorAll('[role=tab]')).find(tab => "
        + "tab.textContent?.trim() === '设计')";
    private static final String CUSTOM_SKIN_TEMPLATE =
        "document.querySelector('[aria-label=\"皮肤模板 奶油桃桃\"]')";
    private static final String CUSTOM_SKIN_BLUEPRINT_TEMPLATE =
        "document.querySelector('[aria-label=\"皮肤模板 工程蓝图\"]')";
    private static final String CUSTOM_SKIN_MY_TAB =
        "Array.from(document.querySelectorAll('[role=tab]')).find(tab => "
        + "tab.textContent?.trim() === '我的')";
    private WebView web;
    @Override protected String successDescription() {
        return "React community entry, save, named custom skin CRUD, Apple custom keyboard design, keyboard geometry reset, scheme visibility fallback, persistence and cross-process IME application";
    }
    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath())) throw new AssertionError("Preferences escaped the preview sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        File skinLibrary = new File(directory, "CustomSkins/library.json");
        if (!skinLibrary.getCanonicalFile().toPath().startsWith(directory.toPath())
                || skinLibrary.getCanonicalFile().equals(preferences.getCanonicalFile()))
            throw new AssertionError("Custom skin library is not independent from preferences");
        byte[] originalSkinLibrary = skinLibrary.exists() ? Files.readAllBytes(skinLibrary.toPath()) : null;
        Files.deleteIfExists(skinLibrary.toPath());
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        Activity activity = null;
        try {
            stage = "React settings load";
            Intent intent = new Intent().setClassName(getTargetContext(), "app.msime.android.MainActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            activity = startActivitySync(intent);
            Activity owner = activity;
            long deadline = SystemClock.uptimeMillis() + 15000;
            do {
                runOnMainSync(() -> web = findWebView(owner.getWindow().getDecorView()));
                if (web != null) break;
                SystemClock.sleep(100);
            } while (SystemClock.uptimeMillis() < deadline);
            if (web == null) throw new AssertionError("Tauri WebView not created");
            awaitJs("!!Array.from(document.querySelectorAll('button')).find(button => "
                + "button.textContent?.trim() === '我的')");
            awaitJs("!!Array.from(document.querySelectorAll('button')).find(button => "
                + "button.textContent?.trim() === '社区')");
            awaitJs("!!(" + PUNCTUATION_CHECKBOX + ")");
            boolean before = "true".equals(js("(" + PUNCTUATION_CHECKBOX + ").checked"));
            stage = "Android system back restores settings page";
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '输入').click(); true");
            awaitJs("document.querySelector('#page-title')?.textContent === '输入'");
            shell("input keyevent 4");
            awaitJs("document.querySelector('#page-title')?.textContent === '首页'");
            stage = "React touch scheme settings";
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '输入').click(); true");
            awaitJs("!!(" + NINE_KEY_TOGGLE + ")");
            // 越南语和藏文在这里总会出现；粤拼和注音只有 APK 带了它们的词库时才出现，所以末尾是按选择器顺序排列的已安装子集。
            awaitJs("(titles => JSON.stringify(titles.slice(0, 11))"
                + " === JSON.stringify(['全拼 26 键','全拼 9 键','小鹤双拼','自然码双拼','微软双拼','首道双拼','86 五笔','日语 9 键','日语 26 键','手写','韩语 26 键'])"
                + " && JSON.stringify(titles.slice(11)) === JSON.stringify(['粤拼 26 键','大千注音','越南语 26 键','藏文 26 键'].filter(title => titles.includes(title)))"
                + " && titles.includes('越南语 26 键') && titles.includes('藏文 26 键'))"
                + "(Array.from(document.querySelectorAll('.touch-keyboard-scheme-select')).map(button => button.textContent.replace('✓', '')))");
            if (!"true".equals(js("(" + QUANPIN_TOGGLE + ").checked")))
                js("(" + QUANPIN_TOGGLE + ").click(); true");
            if (!"true".equals(js("(" + NINE_KEY_TOGGLE + ").checked")))
                js("(" + NINE_KEY_TOGGLE + ").click(); true");
            js("(" + NINE_KEY_SELECT + ").click(); true");
            awaitJs("(" + NINE_KEY_SELECT + ").getAttribute('aria-pressed') === 'true'");
            js("(" + NINE_KEY_TOGGLE + ").click(); true");
            awaitJs("!(" + NINE_KEY_TOGGLE + ").checked && (" + NINE_KEY_SELECT
                + ").disabled && (" + QUANPIN_SELECT + ").getAttribute('aria-pressed') === 'true'");
            awaitJs("!!(" + WUBI_MIXED_PINYIN + ")");
            js("(" + WUBI_MIXED_PINYIN + ").click(); true");
            awaitJs("(" + WUBI_MIXED_PINYIN + ").checked");
            stage = "React keyboard skin settings";
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '屏幕键盘').click(); true");
            awaitJs("!!(" + KEYBOARD_HEIGHT + ")");
            // 我的皮肤 lives on the 主题 page; the keyboard page's 屏幕键盘外观 row is the way there.
            js("document.querySelector('[aria-label=\"屏幕键盘外观\"]').click(); true");
            awaitJs("!!(" + MY_SKIN + ") && !(" + MY_SKIN + ").closest('fieldset').hidden");
            stage = "React custom keyboard skin editor";
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '设计我的皮肤').click(); true");
            awaitJs("!!(" + CUSTOM_SKIN_DESIGN_TAB + ")");
            js("(" + CUSTOM_SKIN_DESIGN_TAB + ").click(); true");
            awaitJs("!!(" + CUSTOM_SKIN_TEMPLATE + ")");
            js("(" + CUSTOM_SKIN_TEMPLATE + ").click(); true");
            awaitJs("document.querySelector('[data-preview-skin=\"custom\"]')?.getAttribute('data-key-shape') === 'pebble'");
            awaitJs("document.querySelector('[data-preview-skin=\"custom\"]')?.getAttribute('data-key-material') === 'raised'");
            stage = "React named custom skin create";
            awaitJs("!Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '保存设计').disabled");
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '保存设计').click(); true");
            awaitJs("!!document.querySelector('[aria-label=\"皮肤名称\"]')");
            setReactInput("document.querySelector('[aria-label=\"皮肤名称\"]')", "设备验收样例");
            awaitJs("!Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '确认保存').disabled");
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '确认保存').click(); true");
            awaitJs("!!document.querySelector('[aria-label=\"应用已保存皮肤 设备验收样例\"]')");
            awaitJs("(" + MY_SKIN + ")?.getAttribute('aria-checked') === 'true'");
            JSONArray createdLibrary = readSkinLibrary(skinLibrary);
            if (createdLibrary.length() != 1
                    || !"设备验收样例".equals(createdLibrary.getJSONObject(0).getString("name"))
                    || !"pebble".equals(createdLibrary.getJSONObject(0).getJSONObject("design").getString("keyShape")))
                throw new AssertionError("Named custom skin create did not reach its independent library");
            if (original == null ? preferences.exists()
                    : !java.util.Arrays.equals(original, Files.readAllBytes(preferences.toPath())))
                throw new AssertionError("Named custom skin mutation changed hot-path preferences");

            stage = "React named custom skin rename through Tauri IPC";
            String createdId = createdLibrary.getJSONObject(0).getString("id");
            js("window.__TAURI_INTERNALS__.invoke('mutate_custom_skin_library',{action:{operation:'rename',id:'"
                + createdId + "',name:'设备验收样例1'}});true");
            awaitSkinLibraryName(skinLibrary, "设备验收样例1");

            stage = "React named custom skin reload";
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '完成').click(); true");
            awaitJs("!document.querySelector('[aria-label=\"自定义皮肤编辑器\"]')");
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '设计我的皮肤').click(); true");
            awaitJs("!!(" + CUSTOM_SKIN_MY_TAB + ")");
            js("(" + CUSTOM_SKIN_MY_TAB + ").click(); true");
            awaitJs("!!document.querySelector('[aria-label=\"应用已保存皮肤 设备验收样例1\"]')");
            stage = "React named custom skin rename persisted";
            if (!"设备验收样例1".equals(readSkinLibrary(skinLibrary).getJSONObject(0).getString("name")))
                throw new AssertionError("Named custom skin rename did not persist");

            stage = "React named custom skin update";
            js("(" + CUSTOM_SKIN_DESIGN_TAB + ").click(); true");
            awaitJs("!!(" + CUSTOM_SKIN_BLUEPRINT_TEMPLATE + ")");
            js("(" + CUSTOM_SKIN_BLUEPRINT_TEMPLATE + ").click(); true");
            js("(" + CUSTOM_SKIN_MY_TAB + ").click(); true");
            awaitJs("document.querySelector('[aria-label=\"用当前设计更新 设备验收样例1\"]')?.disabled === false");
            js("document.querySelector('[aria-label=\"用当前设计更新 设备验收样例1\"]').click(); true");
            awaitJs("!!document.querySelector('[role=alertdialog]')");
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '确认更新').click(); true");
            awaitJs("!document.querySelector('[role=alertdialog]')");
            JSONObject updatedDesign = readSkinLibrary(skinLibrary).getJSONObject(0).getJSONObject("design");
            if (!"glass".equals(updatedDesign.getString("keyMaterial"))
                    || updatedDesign.getInt("pattern") != 2)
                throw new AssertionError("Named custom skin update did not persist");

            stage = "React named custom skin apply";
            js("(" + CUSTOM_SKIN_DESIGN_TAB + ").click(); true");
            js("(" + CUSTOM_SKIN_TEMPLATE + ").click(); true");
            js("(" + CUSTOM_SKIN_MY_TAB + ").click(); true");
            js("document.querySelector('[aria-label=\"应用已保存皮肤 设备验收样例1\"]').click(); true");
            awaitJs("document.querySelector('.touch-skin-editor-preview [data-preview-skin=\"custom\"]')?.getAttribute('data-key-material') === 'glass'");

            stage = "React named custom skin delete";
            awaitJs("document.querySelector('[aria-label=\"删除 设备验收样例1\"]')?.disabled === false");
            js("document.querySelector('[aria-label=\"删除 设备验收样例1\"]').click(); true");
            awaitJs("!!document.querySelector('[role=alertdialog]')");
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '确认删除').click(); true");
            awaitJs("!document.querySelector('[aria-label=\"应用已保存皮肤 设备验收样例1\"]')");
            if (readSkinLibrary(skinLibrary).length() != 0)
                throw new AssertionError("Named custom skin delete did not persist");

            stage = "React restore current custom skin design";
            js("(" + CUSTOM_SKIN_DESIGN_TAB + ").click(); true");
            js("(" + CUSTOM_SKIN_TEMPLATE + ").click(); true");
            js("const use=Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '使用皮肤');if(use)use.click();true");
            stage = "React keyboard height setting";
            js("Array.from(document.querySelectorAll('button')).find(button => button.textContent?.trim() === '屏幕键盘').click(); true");
            awaitJs("!(" + KEYBOARD_HEIGHT + ").closest('fieldset').hidden");
            js("const slider=" + KEYBOARD_HEIGHT + ";"
                + "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(slider,'24');"
                + "slider.dispatchEvent(new Event('input',{bubbles:true}));true");
            awaitJs("(" + KEYBOARD_HEIGHT + ").value === '24'");
            stage = "React save through Tauri";
            js("(" + PUNCTUATION_CHECKBOX + ").click(); true");
            awaitJs("!document.querySelector('button[type=submit]').disabled");
            js("document.querySelector('button[type=submit]').click(); true");
            awaitJs("document.querySelector('[role=status]')?.textContent === '设置已保存。'");
            JSONObject saved = new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8));
            if (saved.getLong("revision") != revision + 1 || saved.getJSONObject("preferences").getBoolean("chinese_punctuation") == before) throw new AssertionError("React save did not reach shared storage");
            JSONObject savedPreferences = saved.getJSONObject("preferences");
            if (savedPreferences.getInt("touch_keyboard_height_adjustment") != 24)
                throw new AssertionError("Keyboard height did not reach shared storage");
            if (!"custom".equals(savedPreferences.getString("global_theme")))
                throw new AssertionError("Keyboard skin did not reach shared storage");
            JSONObject customSkin = savedPreferences.getJSONObject("custom_theme").getJSONObject("keyboard");
            if (!"pebble".equals(customSkin.getString("keyShape"))
                    || !"raised".equals(customSkin.getString("keyMaterial"))
                    || customSkin.getInt("cornerRadius") != 18
                    || customSkin.getInt("pattern") != 3)
                throw new AssertionError("Custom keyboard design did not reach shared storage");
            JSONObject touchSchemes = savedPreferences.getJSONObject("touch_keyboard_schemes");
            if (!"quanpin".equals(touchSchemes.getString("selected")))
                throw new AssertionError("Shared selected scheme did not use the fallback");
            for (int index = 0; index < touchSchemes.getJSONArray("enabled").length(); index++) {
                if ("nine_key".equals(touchSchemes.getJSONArray("enabled").getString(index)))
                    throw new AssertionError("Hidden scheme remained enabled in shared storage");
            }
            if (!"quanpin".equals(savedPreferences.getString("scheme")))
                throw new AssertionError("Engine scheme did not use the fallback");
            if (!"twenty_six_key".equals(savedPreferences.getString("touch_keyboard_layout")))
                throw new AssertionError("Touch layout did not use the fallback");
            if (!savedPreferences.getBoolean("wubi_mixed_pinyin"))
                throw new AssertionError("Wubi mixed-pinyin preference did not reach shared storage");
            stage = "React reset keyboard geometry";
            js("window.confirm = () => true; (" + RESET_KEYBOARD_SETTINGS + ").click(); true");
            awaitJs("(" + KEYBOARD_HEIGHT + ").value === '0' && (" + KEY_SPACING
                + ").value === '60' && (" + ROW_SPACING + ").value === '70' && !("
                + VOICE_SHORTCUT + ").checked");
            awaitJs("!document.querySelector('button[type=submit]').disabled");
            js("document.querySelector('button[type=submit]').click(); true");
            awaitJs("document.querySelector('[role=status]')?.textContent === '设置已保存。'");
            JSONObject resetSaved = new JSONObject(new String(Files.readAllBytes(
                preferences.toPath()), StandardCharsets.UTF_8));
            if (resetSaved.getLong("revision") != saved.getLong("revision") + 1)
                throw new AssertionError("Keyboard geometry reset did not advance preferences revision");
            JSONObject resetPreferences = resetSaved.getJSONObject("preferences");
            for (String key : new String[] {"touch_key_spacing_tenths",
                    "touch_row_spacing_tenths", "touch_keyboard_height_adjustment",
                    "touch_voice_shortcut"}) {
                if (resetPreferences.has(key))
                    throw new AssertionError("Keyboard geometry reset retained " + key);
            }
            stage = "React reload";
            js("(" + RELOAD_BUTTON + ").click(); true");
            awaitJs("!(" + RELOAD_BUTTON + ").disabled && ("
                + PUNCTUATION_CHECKBOX + ").checked === " + !before + " && ("
                + KEYBOARD_HEIGHT + ").value === '0' && (" + KEY_SPACING
                + ").value === '60' && (" + ROW_SPACING + ").value === '70' && !("
                + VOICE_SHORTCUT + ").checked");
            stage = "cross-process system input uses saved preferences";
            shell("ime disable app.msime.android/app.msime.android.MSIMEInputService");
            shell("ime enable app.msime.android/app.msime.android.MSIMEInputService");
            shell("ime set app.msime.android/app.msime.android.MSIMEInputService");
            SystemClock.sleep(1000);
            shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            stage = "cross-process keyboard uses saved skin";
            awaitAnyNode(node -> equalsText("app.msime.android", node.getPackageName())
                && equalsText("切换键盘皮肤；当前我的皮肤", node.getContentDescription()));
            stage = "cross-process punctuation uses saved preferences";
            for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
            tapSymbol(",");
            String expected = before ? "你好," : "你好，";
            await(field("msime-test-plain").and(node -> equalsText(expected, node.getText())));
            stage = "cross-process scheme picker uses shared visibility";
            assertSharedSchemePicker();
            stage = "scheme visibility survives IME restart";
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            shell("ime disable app.msime.android/app.msime.android.MSIMEInputService");
            shell("ime enable app.msime.android/app.msime.android.MSIMEInputService");
            shell("ime set app.msime.android/app.msime.android.MSIMEInputService");
            SystemClock.sleep(1000);
            shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            assertSharedSchemePicker();
        } finally {
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
            if (originalSkinLibrary == null) Files.deleteIfExists(skinLibrary.toPath());
            else { skinLibrary.getParentFile().mkdirs(); publish(skinLibrary, originalSkinLibrary); }
        }
    }
    private void assertSharedSchemePicker() throws Exception {
        String prefix = stage;
        stage = prefix + ": open picker";
        tap(node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("输入方案：全拼 26 键", node.getContentDescription()));
        stage = prefix + ": selected fallback card";
        await(node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("输入方案卡片 全拼 26 键", node.getContentDescription())
            && equalsText("已选中", node.getStateDescription()));
        stage = prefix + ": hidden card absence";
        for (var window : automation.getWindows()) {
            if (find(window.getRoot(), node -> equalsText("app.msime.android", node.getPackageName())
                    && equalsText("输入方案卡片 全拼 9 键", node.getContentDescription())) != null) {
                throw new AssertionError("Hidden scheme remained in the keyboard picker");
            }
        }
        stage = prefix + ": return to keyboard";
        tap(node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("返回键盘", node.getContentDescription()));
    }
    private AccessibilityNodeInfo awaitAnyNode(java.util.function.Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            for (var window : automation.getWindows()) {
                AccessibilityNodeInfo found = findAnyNode(window.getRoot(), match);
                if (found != null) return found;
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected synthetic control was not observed");
    }
    private AccessibilityNodeInfo findAnyNode(AccessibilityNodeInfo node,
                                               java.util.function.Predicate<AccessibilityNodeInfo> match) {
        if (node == null) return null;
        if (match.test(node)) return node;
        for (int index = 0; index < node.getChildCount(); index++) {
            AccessibilityNodeInfo found = findAnyNode(node.getChild(index), match);
            if (found != null) return found;
        }
        return null;
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
        CountDownLatch completed = new CountDownLatch(1);
        AtomicReference<String> result = new AtomicReference<>();
        runOnMainSync(() -> web.evaluateJavascript(expression, value -> { result.set(value); completed.countDown(); }));
        if (!completed.await(15, TimeUnit.SECONDS)) throw new AssertionError("WebView response timed out");
        return result.get();
    }
    private void awaitJs(String condition) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do { if ("true".equals(js(condition))) return; SystemClock.sleep(100); } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected React state was not observed");
    }
    private void setReactInput(String selector, String value) throws Exception {
        js("const input=" + selector + ";"
            + "Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,'" + value + "');"
            + "input.dispatchEvent(new Event('input',{bubbles:true}));true");
        SystemClock.sleep(150);
    }
    private void awaitSkinLibraryName(File file, String expected) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if (file.exists()) {
                JSONArray items = readSkinLibrary(file);
                if (items.length() == 1 && expected.equals(items.getJSONObject(0).getString("name")))
                    return;
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Named custom skin command did not persist");
    }
    private JSONArray readSkinLibrary(File file) throws Exception {
        return new JSONArray(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
        }
    }
    private void publish(File file, byte[] contents) throws Exception {
        AtomicFile target = new AtomicFile(file);
        FileOutputStream output = target.startWrite();
        try { output.write(contents); target.finishWrite(output); }
        catch (Exception error) { target.failWrite(output); throw error; }
    }
}
