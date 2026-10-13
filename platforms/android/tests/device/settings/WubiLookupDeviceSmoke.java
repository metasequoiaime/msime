package app.msime.android.test;

import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 五笔查码页（#6789）的设备验收：启用五笔、选 98 码表后，「输入」页出现「五笔查码」，打开它查「段舞」，列出两个字各自的 86 和 98 编码，当前码表那一列标「（当前）」；查「输入法」时整段词也有一行；再查一段没有汉字的文字，提示没有可以查的汉字。
 *
 * <p>查码在设置应用进程里只读打开安装时解出来的 `msime-wubi.db`（词库请求 `wubi_codes`）。这是 AGENTS.md 要求在模拟器上跑过的文件读取：JVM 冒烟和 CI 的编译都碰不到应用私有目录的权限。期望的编码取自随包词库 dict-v2.0.14：段 86 `wdm wdmc`、98 `th thdc`，舞 86 `rlg rlgh`、98 `tgl tglg`，两张表不一样；输入法两张表都是 `ltif`，所以两列各自的来源都验到了。
 */
public final class WubiLookupDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "Wubi lookup row on the input page, 86 and 98 codes read from the packaged table, current table marked, and the no-character notice";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped the preview sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
            .put("wubi_profile", "wubi98")
            .put("touch_keyboard_schemes", new JSONObject().put("selected", "quanpin")
                .put("enabled", new JSONArray().put("quanpin").put("wubi")));
        JSONObject snapshot = new JSONObject().put("format_version", 1).put("revision", revision + 1)
            .put("preferences", values);
        try {
            stage = "Wubi preferences";
            publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page TYPING");

            stage = "input page lists the lookup";
            tap(settingsText("中文").and(AccessibilityNodeInfo::isClickable));
            tap(settingsText("五笔查码"));
            stage = "lookup page";
            AccessibilityNodeInfo field = await(settingsDescribed("要查五笔编码的文字"));
            setText(field, "段舞");
            tap(settingsText("查码").and(AccessibilityNodeInfo::isEnabled));

            stage = "codes for each character";
            await(settingsDescribed("段，86 五笔 wdm wdmc，98 五笔（当前） th thdc"));
            await(settingsDescribed("舞，86 五笔 rlg rlgh，98 五笔（当前） tgl tglg"));

            stage = "a phrase gets its own row";
            setText(await(settingsDescribed("要查五笔编码的文字")), "输入法");
            tap(settingsText("查码").and(AccessibilityNodeInfo::isEnabled));
            await(settingsDescribed("输入法，86 五笔 ltif，98 五笔（当前） ltif"));
            await(settingsDescribed("法，86 五笔 if ifc ifcy，98 五笔（当前） if ifc ifcy"));

            stage = "text without characters";
            setText(await(settingsDescribed("要查五笔编码的文字")), "abc，123");
            tap(settingsText("查码").and(AccessibilityNodeInfo::isEnabled));
            await(settingsText("没有可以查的汉字。"));
        } catch (Exception | AssertionError error) {
            shellDiscard("screencap -p /data/local/tmp/msime-wubi-lookup-failure.png");
            throw error;
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    /** 设置应用里文字正好是 `text` 的控件。 */
    private Predicate<AccessibilityNodeInfo> settingsText(String text) {
        return node -> equalsText("app.msime.android", node.getPackageName()) && equalsText(text, node.getText());
    }

    private Predicate<AccessibilityNodeInfo> settingsDescribed(String description) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText(description, node.getContentDescription());
    }

    private void setText(AccessibilityNodeInfo field, String text) {
        Bundle arguments = new Bundle();
        arguments.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text);
        if (!field.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, arguments))
            throw new AssertionError("Could not enter the lookup text");
        SystemClock.sleep(200);
    }


}
