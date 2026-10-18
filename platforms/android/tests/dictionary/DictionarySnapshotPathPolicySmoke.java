import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.stream.Stream;

/** 快照队列交给 Rust 以后（#8201 迁移计划 5d），Java 这边还留着两道路径检查：Tauri 合包 `enqueueSnapshot` 的来源文件经 DictionarySnapshotPathPolicy.privateSource，`:ime` 每次空闲把暂存目录交给 Rust 前先经 DictionarySnapshotWorker.ensureSafeDirectory。两处都是包内可见，只能反射调用。 */
public final class DictionarySnapshotPathPolicySmoke {
    private static void check(boolean condition) {
        if (!condition) throw new AssertionError();
    }

    private static Object invoke(Method method, Object... args) throws IOException {
        try {
            return method.invoke(null, args);
        } catch (InvocationTargetException error) {
            if (error.getCause() instanceof IOException io) throw io;
            throw new AssertionError(error.getCause());
        } catch (IllegalAccessException error) {
            throw new AssertionError(error);
        }
    }

    private static boolean rejects(Method method, Object... args) {
        try {
            invoke(method, args);
            return false;
        } catch (IOException expected) {
            return true;
        }
    }

    public static void main(String[] args) throws Exception {
        Method privateSource = Class.forName("app.msime.android.DictionarySnapshotPathPolicy")
            .getDeclaredMethod("privateSource", Path.class, Path.class, String.class);
        privateSource.setAccessible(true);
        Method ensureSafeDirectory = Class.forName("app.msime.android.DictionarySnapshotWorker")
            .getDeclaredMethod("ensureSafeDirectory", Path.class);
        ensureSafeDirectory.setAccessible(true);

        Path root = Files.createTempDirectory("msime-snapshot-paths-");
        try {
            Path files = root.resolve("files");
            Path queue = files.resolve("bootstrap/state/dictionary-snapshots");
            Files.createDirectories(queue);

            // 队列目录里的普通文件照常放行，下面的拒绝才说明是链接被拦下，而不是什么都拒绝。
            Path plain = Files.writeString(queue.resolve("plain.ndjson"), "record\n");
            check(plain.equals(invoke(privateSource, files, queue, plain.toString())));

            // 队列目录里有一级是指向 files 以外的链接：链接那头真有这个文件，最后一级不跟随链接的检查看得到它，只有逐级检查链接才拦得下。
            Path sourceOutside = Files.createDirectory(root.resolve("source-outside"));
            Path outsideFixture = Files.writeString(sourceOutside.resolve("fixture.ndjson"), "outside\n");
            Files.createSymbolicLink(queue.resolve("linked"), sourceOutside);
            check(rejects(privateSource, files, queue, queue.resolve("linked/fixture.ndjson").toString()));
            check("outside\n".equals(Files.readString(outsideFixture)));
            try (Stream<Path> entries = Files.list(sourceOutside)) {
                check(entries.count() == 1);
            }

            // 最后一级本身是链接、不在队列目录里、写成 .. 绕出去，都不收。
            Files.createSymbolicLink(queue.resolve("leaf.ndjson"), outsideFixture);
            check(rejects(privateSource, files, queue, queue.resolve("leaf.ndjson").toString()));
            check(rejects(privateSource, files, queue, outsideFixture.toString()));
            check(rejects(privateSource, files, queue, queue.resolve("../../../../source-outside/fixture.ndjson").toString()));
            check(rejects(privateSource, files, queue, null));

            // 暂存目录：正常路径建得出来。
            Path staging = files.resolve("bootstrap/state/dictionary-snapshot-staging");
            invoke(ensureSafeDirectory, staging);
            check(Files.isDirectory(staging));

            // 暂存目录本身或它的上一级是指向别处的链接时抛 IOException，也不能经链接在那头建出目录。
            Path stagingOutside = Files.createDirectory(root.resolve("staging-outside"));
            Path stagingLink = files.resolve("staging-link");
            Files.createSymbolicLink(stagingLink, stagingOutside);
            check(rejects(ensureSafeDirectory, stagingLink));
            check(rejects(ensureSafeDirectory, stagingLink.resolve("nested")));
            check(!Files.exists(stagingOutside.resolve("nested")));
            check(rejects(ensureSafeDirectory, (Object) null));

            System.out.println("Android dictionary snapshot source and staging paths reject symlinks");
        } finally {
            try (Stream<Path> paths = Files.walk(root)) {
                paths.sorted(Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (IOException error) { throw new IllegalStateException(error); }
                });
            }
        }
    }
}
