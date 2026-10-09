package app.msime.android;

import java.io.FileNotFoundException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;

/** 把备份写入文件选择器给的空文档；文档提供方可能返回已有文档。 */
public final class BackupDestinationWriter {
    private BackupDestinationWriter() {}

    public interface Destination {
        InputStream openForRead() throws IOException;
        OutputStream openForWrite() throws IOException;
    }

    public static void write(Path archive, Destination destination) throws IOException {
        try (InputStream current = destination.openForRead()) {
            if (current == null) throw new FileNotFoundException("backup destination unreadable");
            if (current.read() != -1) throw new IOException("backup destination is not empty");
        }
        try (OutputStream output = destination.openForWrite()) {
            if (output == null) throw new FileNotFoundException("backup destination unwritable");
            Files.copy(archive, output);
        }
    }
}
