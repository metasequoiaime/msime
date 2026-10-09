import app.msime.android.BackupDestinationWriter;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

/** 备份导出只写入空文档，提供方返回已有内容时保留原件。 */
public final class BackupDestinationWriterSmoke {
    private static final class Document implements BackupDestinationWriter.Destination {
        private byte[] contents;
        private boolean writeOpened;

        Document(byte[] contents) { this.contents = contents; }

        @Override public InputStream openForRead() { return new ByteArrayInputStream(contents); }

        @Override public OutputStream openForWrite() {
            writeOpened = true;
            return new ByteArrayOutputStream() {
                @Override public void close() throws IOException {
                    super.close();
                    contents = toByteArray();
                }
            };
        }
    }

    public static void main(String[] args) throws Exception {
        Path archive = Files.createTempFile("msime-backup-destination", ".zip");
        try {
            byte[] backup = {1, 2, 3, 4};
            Files.write(archive, backup);
            Document existing = new Document(new byte[] {9, 8, 7});
            try {
                BackupDestinationWriter.write(archive, existing);
                throw new AssertionError("an existing backup was overwritten");
            } catch (IOException expected) {
                if (existing.writeOpened || !Arrays.equals(existing.contents, new byte[] {9, 8, 7}))
                    throw new AssertionError("an existing backup was changed");
            }
            Document fresh = new Document(new byte[0]);
            BackupDestinationWriter.write(archive, fresh);
            if (!Arrays.equals(fresh.contents, backup)) throw new AssertionError("an empty document was not filled");
        } finally {
            Files.deleteIfExists(archive);
        }
        System.out.println("Android backup destination writer passed");
    }
}
