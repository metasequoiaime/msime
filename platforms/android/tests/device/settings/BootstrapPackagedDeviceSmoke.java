package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.content.ContextWrapper;
import android.os.Bundle;
import android.system.Os;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import org.json.JSONObject;

/** 用隔离根和实际 JNI 验收已有瘦包配置补齐 APK 自带按需组，不修改输入法的真实配置。 */
public final class BootstrapPackagedDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        Path sandbox = null;
        Throwable failure = null;
        try {
            Context target = getTargetContext();
            sandbox = Files.createTempDirectory(target.getCacheDir().toPath(), "bootstrap-packaged-");
            final File root = sandbox.toFile();
            Context context = new ContextWrapper(target) {
                @Override public File getFilesDir() { return root; }
            };
            ClassLoader loader = target.getClassLoader();
            Class<?> bootstrap = loader.loadClass("app.msime.android.Bootstrap");
            Class<?> nativeClient = loader.loadClass("app.msime.android.NativeClient");
            File resources = new File(root, "bootstrap/resources");
            var extract = bootstrap.getDeclaredMethod("extractDictionary", Context.class, File.class);
            extract.setAccessible(true);
            extract.invoke(null, context, resources);
            List<Path> optional = new ArrayList<>();
            for (String name : List.of("msime-wubi.db", "msime-japanese.dat",
                    "msime-mozc_dictionary_oss_README.txt", "msime-mozc_LICENSE.txt")) {
                Path file = resources.toPath().resolve(name);
                if (Files.exists(file)) {
                    optional.add(file);
                    Files.delete(file);
                }
            }
            check(optional.stream().anyMatch(path -> path.endsWith("msime-wubi.db")), "本专项必须使用带五笔码表的真实 APK");
            JSONObject request = new JSONObject().put("resources", resources.getAbsolutePath())
                .put("state_root", new File(root, "bootstrap/state").getAbsolutePath());
            JSONObject prepared = new JSONObject((String) nativeClient.getMethod("prepareHost", String.class)
                .invoke(null, request.toString()));
            check(prepared.optBoolean("ok"), "共享层必须允许瘦包的完整按需组缺席");
            Path configuration = root.toPath().resolve("runtime-options.json");
            Files.write(configuration, prepared.getJSONObject("value").toString().getBytes(StandardCharsets.UTF_8));
            check(Boolean.FALSE.equals(bootstrap.getMethod("prepare", Context.class).invoke(null, context)),
                "已有瘦包配置不能当成首次安装");
            List<Long> inodes = new ArrayList<>();
            for (Path file : optional) {
                check(Files.isRegularFile(file), "完整 APK 的按需文件必须补解包");
                check(Files.size(file) > 0, "补解包文件不能为空");
                inodes.add(Os.stat(file.toString()).st_ino);
            }
            // 真实共享刷新应能继续消费同一个资源根，不通过重建配置绕过问题。
            JSONObject refreshed = new JSONObject(new String(Files.readAllBytes(configuration), StandardCharsets.UTF_8));
            check(resources.getCanonicalFile().equals(new File(refreshed.getString("resources")).getCanonicalFile()),
                "补齐后资源根保持原值");
            bootstrap.getMethod("prepare", Context.class).invoke(null, context);
            for (int index = 0; index < optional.size(); index++) {
                check(Os.stat(optional.get(index).toString()).st_ino == inodes.get(index),
                    "后续启动不能重复解包完整按需组");
            }
            Class<?> packs = loader.loadClass("app.msime.android.ResourcePacks");
            var adopt = packs.getDeclaredMethod("adopt", File.class, String.class, File.class);
            adopt.setAccessible(true);
            File installed = (File) adopt.invoke(null, root, "wubi", resources);
            Path table = resources.toPath().resolve("msime-wubi.db");
            check(!Files.exists(table), "实际收编必须移走随包码表");
            File packedTable = new File(installed, "msime-wubi.db");
            long packedInode = Os.stat(packedTable.getAbsolutePath()).st_ino;
            bootstrap.getMethod("prepare", Context.class).invoke(null, context);
            check(!Files.exists(table), "已有有效资源包时不能再复制随包码表");
            check(Os.stat(packedTable.getAbsolutePath()).st_ino == packedInode,
                "启动不能替换已下载资源包");
        } catch (Throwable error) { failure = error; }
        finally {
            if (sandbox != null) {
                try (var paths = Files.walk(sandbox)) {
                    for (Path path : paths.sorted(Comparator.reverseOrder()).collect(java.util.stream.Collectors.toList())) Files.delete(path);
                } catch (Throwable error) {
                    if (failure == null) failure = error;
                    else failure.addSuppressed(error);
                }
            }
        }
        if (failure == null) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: bundled optional groups restore existing configuration and remain stable\n");
            finish(Activity.RESULT_OK, result);
        } else {
            Throwable cause = failure;
            while (cause.getCause() != null) cause = cause.getCause();
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: bundled optional groups (" + cause.getClass().getSimpleName()
                + ")" + (cause instanceof AssertionError ? " " + cause.getMessage() : "") + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
