package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Bundle;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** 在宿主 UID 中保存与切换图标，结束后由独立编辑器 UID 检查真实页面生命周期。 */
public class HostIconPreparation extends Instrumentation {
    private Bundle arguments;

    @Override public void onCreate(Bundle args) {
        super.onCreate(args);
        arguments = args == null ? new Bundle() : args;
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            Context context = getTargetContext();
            var onboarding = context.getSharedPreferences("msime_onboarding_v1", Context.MODE_PRIVATE);
            File backup = new File(context.getCacheDir(), "host-icon-test-backup.json");
            Class<?> styles = Class.forName("app.msime.android.AppIconStyle");
            Class<?> icons = Class.forName("app.msime.android.home.AppIcons");
            String icon = arguments.getString("icon");
            String inspect = arguments.getString("inspect");
            if (inspect != null) {
                verify(context, styles, inspect);
                if (!inspect.equals(((Enum<?>) icons.getMethod("selected", Context.class).invoke(null, context)).name()))
                    throw new AssertionError("Existing color was not preserved");
            } else if (icon != null) {
                if (!backup.isFile()) throw new AssertionError("Missing test backup");
                select(icons, styles, context, icon);
                if (!"false".equals(arguments.getString("verify"))) verify(context, styles, icon);
            } else if ("true".equals(arguments.getString("restore"))) {
                JSONObject saved = new JSONObject(new String(Files.readAllBytes(backup.toPath()), StandardCharsets.UTF_8));
                select(icons, styles, context, saved.getString("icon"));
                var edit = onboarding.edit();
                if (saved.has("seen")) edit.putBoolean("seen", saved.getBoolean("seen"));
                else edit.remove("seen");
                if (!edit.commit()) throw new AssertionError("Onboarding restore failed");
                Files.delete(backup.toPath());
            } else {
                if (backup.exists()) throw new AssertionError("Test backup already exists");
                Class.forName("app.msime.android.Bootstrap").getMethod("prepare", Context.class).invoke(null, context);
                JSONObject saved = new JSONObject();
                saved.put("icon", ((Enum<?>) icons.getMethod("selected", Context.class).invoke(null, context)).name());
                if (onboarding.contains("seen")) saved.put("seen", onboarding.getBoolean("seen", false));
                AtomicFile target = new AtomicFile(backup);
                FileOutputStream output = target.startWrite();
                try {
                    output.write(saved.toString().getBytes(StandardCharsets.UTF_8));
                    target.finishWrite(output);
                } catch (Exception error) {
                    target.failWrite(output);
                    throw error;
                }
                if (!onboarding.edit().putBoolean("seen", true).commit()) throw new AssertionError("Onboarding prepare failed");
            }
            result.putString("stream", "MSIME_ICON_PREPARATION_PASSED\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_ICON_PREPARATION_FAILED: " + error.getClass().getSimpleName()
                + (error instanceof AssertionError ? " " + error.getMessage() : "") + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private static void select(Class<?> icons, Class<?> styles, Context context, String name) throws Exception {
        Object style = styles.getField(name).get(null);
        if (!Boolean.TRUE.equals(icons.getMethod("select", Context.class, styles).invoke(null, context, style)))
            throw new AssertionError("Icon selection failed");
    }

    private static void verify(Context context, Class<?> styles, String name) throws Exception {
        PackageManager packages = context.getPackageManager();
        ComponentName home = new ComponentName(context.getPackageName(), "app.msime.android.home.HomeActivity");
        if (!packages.getActivityInfo(home, 0).enabled) throw new AssertionError("Real host disabled");
        String expected = (String) styles.getMethod("component").invoke(styles.getField(name).get(null));
        Intent launcher = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER).setPackage(context.getPackageName());
        var entries = packages.queryIntentActivities(launcher, 0);
        if (entries.size() != 1 || !expected.equals(entries.get(0).activityInfo.name))
            throw new AssertionError("Launcher aliases are not unique");
    }
}
