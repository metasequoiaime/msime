package app.msime.android;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;

/**
 * 宿主对存储路径唯一的一处符号链接检查，是 {@code crates/path-trust} 的 Java 版本：被人放进去的链接不能把键盘的写入重定向到别处，但系统自己放在每个应用存储路径上的链接必须放行。
 *
 * <p>Android 11 起隔离应用数据，在应用自己的 mount namespace 里 {@code /data/user/0} 是指向 {@code /data/data} 的链接；主用户的 {@code Context.getFilesDir()} 会经过它，而 {@code adb shell} 里看不到。macOS 的 {@code /tmp} 和 {@code /var} 是指向 {@code /private} 的链接，JVM 冒烟测试的临时目录就在那里。每条链接只有在目标完全一致时才受信任。这份清单必须与 {@code crates/path-trust/src/lib.rs} 里的 {@code SYSTEM_ALIASES} 保持一致。
 */
public final class SafePaths {
    private static final String[][] SYSTEM_ALIASES = {
        {"/data/user/0", "/data/data"},
        {"/tmp", "/private/tmp"},
        {"/var", "/private/var"},
    };

    private SafePaths() {}

    /** 判断 {@code path} 是否是系统链接之一，并且从它读出的 {@code target} 解析后正好是该链接唯一受信任的指向。 */
    static boolean trustedSystemAliasTarget(Path path, Path target) {
        for (String[] alias : SYSTEM_ALIASES) {
            if (!path.equals(Path.of(alias[0]))) continue;
            Path parent = path.getParent() == null ? path.getRoot() : path.getParent();
            return parent.resolve(target).normalize().equals(Path.of(alias[1]));
        }
        return false;
    }

    static boolean isTrustedSystemAlias(Path path) {
        try {
            return trustedSystemAliasTarget(path, Files.readSymbolicLink(path));
        } catch (IOException | UnsupportedOperationException | SecurityException error) {
            return false;
        }
    }

    /** 拒绝 {@code path} 任何一级上的符号链接（包括最后一级），唯一的例外是最后一级之上至多一个受信任的系统链接。不存在的层级可以接受，调用方正要创建它们。 */
    public static void rejectSymlinkComponents(Path path) throws IOException {
        if (path == null) throw new IOException("path unavailable");
        Path absolute = path.toAbsolutePath().normalize();
        Path current = absolute.getRoot();
        if (current == null) throw new IOException("path unavailable");
        boolean sawSystemAlias = false;
        int remaining = absolute.getNameCount();
        for (Path component : absolute) {
            current = current.resolve(component);
            remaining--;
            if (!Files.isSymbolicLink(current)) continue;
            if (remaining == 0 || sawSystemAlias || !isTrustedSystemAlias(current))
                throw new IOException("path contains a symbolic link");
            sawSystemAlias = true;
        }
    }

    /** 先确认没有链接重定向 {@code directory} 再创建它，创建后再检查一次。 */
    public static void ensureDirectory(Path directory) throws IOException {
        rejectSymlinkComponents(directory);
        Path absolute = directory.toAbsolutePath().normalize();
        if (Files.exists(absolute, LinkOption.NOFOLLOW_LINKS)
                && !Files.isDirectory(absolute, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("directory unavailable");
        Files.createDirectories(absolute);
        rejectSymlinkComponents(absolute);
        if (!Files.isDirectory(absolute, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("directory unavailable");
    }
}
