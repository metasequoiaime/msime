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
    private boolean slim;
    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        slim = arguments != null && "true".equals(arguments.getString("slim"));
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
            check(slim ? optional.isEmpty() : optional.stream().anyMatch(path -> path.endsWith("msime-wubi.db")),
                "专项参数必须与实际 APK 的随包按需组一致");
            JSONObject request = new JSONObject().put("resources", resources.getAbsolutePath())
                .put("state_root", new File(root, "bootstrap/state").getAbsolutePath());
            JSONObject prepared = new JSONObject((String) nativeClient.getMethod("prepareHost", String.class)
                .invoke(null, request.toString()));
            check(prepared.optBoolean("ok"), "共享层必须允许瘦包的完整按需组缺席");
            Path configuration = root.toPath().resolve("runtime-options.json");
            Files.write(configuration, prepared.getJSONObject("value").toString().getBytes(StandardCharsets.UTF_8));
            Path interrupted = resources.toPath().resolve(".msime-pinyin.db.123456789.staging");
            Files.write(interrupted, new byte[] {1, 2, 3});
            check(Boolean.FALSE.equals(bootstrap.getMethod("prepare", Context.class).invoke(null, context)),
                "已有瘦包配置不能当成首次安装");
            check(!Files.exists(interrupted), "被打断的原子复制暂存文件必须清理");
            if (slim) {
                for (String name : List.of("msime-wubi.db", "msime-japanese.dat"))
                    check(!Files.exists(resources.toPath().resolve(name)), "瘦包不能补出未随包的按需文件");
                bootstrap.getMethod("prepare", Context.class).invoke(null, context);
                check(!Files.exists(root.toPath().resolve("bootstrap/state/resource-packs/wubi")),
                    "正常瘦包启动不能安装或下载五笔资源包");
            } else {
                checkLookup(nativeClient, configuration);
                check(!Files.exists(root.toPath().resolve("bootstrap/wubi-merge-pending")),
                    "重新准备成功后必须清掉五笔合表标记，稳定启动不再重放日志");
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
                // 用真实共享准备建立宿主内部更新目录布局，缺组按资源包补齐，不能写入下载代次。
                Path updated = root.toPath().resolve("bootstrap/state/dictionary-updates/generations/synthetic");
                Files.createDirectories(updated.getParent());
                Files.move(resources.toPath(), updated);
                deleteTree(installed.toPath());
                JSONObject updatedRequest = new JSONObject().put("resources", updated.toString())
                    .put("state_root", new File(root, "bootstrap/state").getAbsolutePath());
                JSONObject updatedPrepared = new JSONObject((String) nativeClient.getMethod("prepareHost", String.class)
                    .invoke(null, updatedRequest.toString()));
                check(updatedPrepared.optBoolean("ok"), "更新目录布局必须由实际共享层准备");
                Files.write(configuration, updatedPrepared.getJSONObject("value").toString().getBytes(StandardCharsets.UTF_8));
                bootstrap.getMethod("prepare", Context.class).invoke(null, context);
                check(!Files.exists(updated.resolve("msime-wubi.db")), "不能向下载代次补写随包资源");
                check(Files.isRegularFile(packedTable.toPath()), "内部更新布局必须收编随包码表");
                checkLookup(nativeClient, configuration);
                long updateInode = Os.stat(packedTable.getAbsolutePath()).st_ino;
                bootstrap.getMethod("prepare", Context.class).invoke(null, context);
                check(Os.stat(packedTable.getAbsolutePath()).st_ino == updateInode,
                    "更新布局重复启动不能替换资源包");
                // 外部目录即使也用本宿主状态根，也不推断成安装包或下载代次。
                deleteTree(installed.toPath());
                Path external = root.toPath().resolve("custom-resources");
                Files.move(updated, external);
                JSONObject externalPrepared = new JSONObject((String) nativeClient.getMethod("prepareHost", String.class)
                    .invoke(null, new JSONObject().put("resources", external.toString())
                        .put("state_root", new File(root, "bootstrap/state").getAbsolutePath()).toString()));
                check(externalPrepared.optBoolean("ok"), "自定义目录必须是实际共享层可接受的配置");
                Files.write(configuration, externalPrepared.getJSONObject("value").toString().getBytes(StandardCharsets.UTF_8));
                bootstrap.getMethod("prepare", Context.class).invoke(null, context);
                check(!Files.exists(external.resolve("msime-wubi.db")) && !Files.exists(packedTable.toPath()),
                    "不能向自定义资源目录补文件或替其收编");
                check(!Files.exists(resources.toPath().resolve("msime-wubi.db")), "自定义资源配置不能触发随包补齐");
            }
        } catch (Throwable error) { failure = error; }
        finally {
            if (sandbox != null) {
                try {
                    deleteTree(sandbox);
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

    private static void deleteTree(Path directory) throws Exception {
        try (var paths = Files.walk(directory)) {
            for (Path path : paths.sorted(Comparator.reverseOrder()).collect(java.util.stream.Collectors.toList())) Files.delete(path);
        }
    }

    private static void checkLookup(Class<?> nativeClient, Path configuration) throws Exception {
        JSONObject options = new JSONObject(new String(Files.readAllBytes(configuration), StandardCharsets.UTF_8));
        JSONObject request = new JSONObject().put("options", options)
            .put("action", new JSONObject().put("operation", "wubi_codes").put("text", "段舞"));
        JSONObject result = new JSONObject((String) nativeClient.getMethod("personalDictionaryRequest", String.class)
            .invoke(null, request.toString()));
        check(result.optBoolean("ok"), "实际查码请求必须成功");
        JSONObject first = result.getJSONObject("value").getJSONArray("characters").getJSONObject(0);
        check(first.getJSONArray("wubi86").toString().contains("wdmc")
            && first.getJSONArray("wubi98").toString().contains("thdc"), "补齐后实际查码必须读到两份码表");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
