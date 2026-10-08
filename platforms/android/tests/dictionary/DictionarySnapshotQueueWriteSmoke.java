package app.msime.android;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.channels.WritableByteChannel;

public final class DictionarySnapshotQueueWriteSmoke {
    private static final class PartialChannel implements WritableByteChannel {
        private boolean open = true;
        private final int maximum;
        private int writes;

        PartialChannel(int maximum) { this.maximum = maximum; }

        @Override public int write(ByteBuffer source) {
            int count = Math.min(maximum, source.remaining());
            source.position(source.position() + count);
            writes++;
            return count;
        }

        @Override public boolean isOpen() { return open; }
        @Override public void close() { open = false; }
        int writes() { return writes; }
    }

    public static void main(String[] args) throws IOException {
        PartialChannel channel = new PartialChannel(2);
        ByteBuffer bytes = ByteBuffer.wrap(new byte[] { 1, 2, 3, 4, 5 });
        DictionarySnapshotQueue.writeFully(channel, bytes);
        if (bytes.hasRemaining() || channel.writes() < 3) throw new AssertionError();
        System.out.println("Android snapshot queue: partial channel writes are completed");
    }
}
