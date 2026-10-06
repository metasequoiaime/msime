import app.msime.android.HttpBodyPolicy;
import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

public final class HttpBodyPolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) throws Exception {
        check(new String(HttpBodyPolicy.readBounded(
            new ByteArrayInputStream("hello".getBytes(StandardCharsets.UTF_8)), 5),
            StandardCharsets.UTF_8).equals("hello"));
        check(HttpBodyPolicy.readBounded(
            new ByteArrayInputStream("hello".getBytes(StandardCharsets.UTF_8)), 4) == null);
        check(HttpBodyPolicy.readBounded(new ByteArrayInputStream(new byte[0]), 4).length == 0);
        // 按整体时限读：时限内读完照常返回，超过字节上限或读完前已过时限都返回 null。
        long later = System.nanoTime() + 60_000_000_000L;
        check("ok".equals(new String(HttpBodyPolicy.readWithin(
            new ByteArrayInputStream("ok".getBytes(StandardCharsets.UTF_8)), 16, later), StandardCharsets.UTF_8)));
        check(HttpBodyPolicy.readWithin(
            new ByteArrayInputStream(new byte[17]), 16, later) == null);
        // 每次只给一个字节、间隔 30 ms：每次读都不空闲超时，但整体在 100 ms 的时限之后才读完。
        InputStream trickle = new InputStream() {
            private int left = 10;
            @Override public int read() { return -1; }
            @Override public int read(byte[] buffer, int offset, int length) {
                if (left-- <= 0) return -1;
                try { Thread.sleep(30); } catch (InterruptedException error) { throw new AssertionError(error); }
                buffer[offset] = 'x';
                return 1;
            }
        };
        check(HttpBodyPolicy.readWithin(trickle, 16, System.nanoTime() + 100_000_000L) == null);
        System.out.println("Android bounded HTTP body policy passed");
    }
}
