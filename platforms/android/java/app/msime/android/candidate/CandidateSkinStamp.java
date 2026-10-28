package app.msime.android;

import java.io.IOException;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.List;

/** 偏好文件与候选包的元数据戳：只 stat，不读内容。戳相同就当文件没变，偏好轮询据此跳过整套重读与重新校验。 */
public final class CandidateSkinStamp {
    /** 一个包目录下最多数这么多条；超出时不给戳，调用方每次照常整套读。 */
    static final int MAX_PACKAGE_ENTRIES = 256;

    private CandidateSkinStamp() {}

    /** 普通文件的 inode、修改时间和大小（与 AndroidLocalSettings 相同的判据）；不存在、不是普通文件或读不出时为 null。 */
    public static Object file(Path file) {
        BasicFileAttributes attributes = attributes(file);
        if (attributes == null || !attributes.isRegularFile()) return null;
        return Arrays.asList(attributes.fileKey(), attributes.lastModifiedTime(), attributes.size());
    }

    /**
     * 皮肤根目录本身，加上指名的每个包目录里的全部条目（相对路径、类型、inode、修改时间、大小）。
     *
     * <p>根目录的修改时间盖住包的安装、删除与改名；逐个条目盖住包内文件的原地改写和原子替换，所以只换图片、不动清单也会被看到。包目录不存在也记进戳里，之后装上时戳就变了。条目太多或读不出时返回 null，调用方按「变了」处理。
     */
    public static Object packages(Path root, Collection<String> ids) {
        List<Object> stamp = new ArrayList<>();
        BasicFileAttributes rootAttributes = attributes(root);
        stamp.add(rootAttributes == null ? "missing"
            : Arrays.asList(rootAttributes.isDirectory(), rootAttributes.fileKey(), rootAttributes.lastModifiedTime()));
        for (String id : ids) {
            stamp.add(id);
            // 不是单层目录名的 id 共享解析器本来就拒绝，这里只记下它，不去拼路径。
            if (id.isEmpty() || id.equals(".") || id.equals("..") || id.indexOf('/') >= 0 || id.indexOf('\\') >= 0) {
                stamp.add("invalid");
                continue;
            }
            List<Object> entries = entries(root.resolve(id));
            if (entries == null) return null;
            stamp.add(entries);
        }
        return stamp;
    }

    private static List<Object> entries(Path directory) {
        BasicFileAttributes top = attributes(directory);
        if (top == null) return List.of("missing");
        List<Object> result = new ArrayList<>();
        result.add(Arrays.asList(top.isDirectory(), top.fileKey(), top.lastModifiedTime()));
        if (!top.isDirectory()) return result;
        ArrayDeque<Path> pending = new ArrayDeque<>();
        pending.add(directory);
        int count = 0;
        while (!pending.isEmpty()) {
            Path current = pending.removeFirst();
            List<Path> children = new ArrayList<>();
            try (DirectoryStream<Path> stream = Files.newDirectoryStream(current)) {
                for (Path child : stream) {
                    if (++count > MAX_PACKAGE_ENTRIES) return null;
                    children.add(child);
                }
            } catch (IOException | RuntimeException error) {
                return null;
            }
            children.sort(null);
            for (Path child : children) {
                BasicFileAttributes attributes = attributes(child);
                if (attributes == null) return null;
                result.add(Arrays.asList(directory.relativize(child).toString(), attributes.isDirectory(),
                    attributes.isRegularFile(), attributes.fileKey(), attributes.lastModifiedTime(), attributes.size()));
                if (attributes.isDirectory()) pending.add(child);
            }
        }
        return result;
    }

    private static BasicFileAttributes attributes(Path path) {
        try {
            return Files.readAttributes(path, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
        } catch (IOException | RuntimeException error) {
            return null;
        }
    }
}
