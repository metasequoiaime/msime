package app.msime.android;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.PorterDuff;
import android.graphics.PorterDuffXfermode;
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.zip.CRC32;

/** APNG 子帧的有界解码；Android 原生 PNG 解码器只保证默认图。 */
public final class CandidateApngFrames {
    private static final byte[] SIGNATURE = {(byte) 137, 80, 78, 71, 13, 10, 26, 10};
    private final Bitmap[] frames;
    private final long[] ends;
    private final long loops;

    private CandidateApngFrames(Bitmap[] frames, long[] ends, long loops) {
        this.frames = frames;
        this.ends = ends;
        this.loops = loops;
    }

    public int size() { return frames.length; }
    Bitmap frame(int index) { return frames[index]; }
    public long durationMillis() { return ends[ends.length - 1]; }
    public long loops() { return loops; }
    public Bitmap frameAt(long elapsed, boolean motion) {
        if (!motion) return frames[0];
        long duration = durationMillis();
        elapsed = Math.max(0, elapsed);
        if (loops != 0 && elapsed / duration >= loops) return frames[frames.length - 1];
        long time = elapsed % duration;
        for (int i = 0; i < ends.length; i++) if (time < ends[i]) return frames[i];
        return frames[frames.length - 1];
    }

    /** 普通 PNG 返回 null；损坏、越界或不完整的 APNG 抛异常，不能悄悄只画默认图。 */
    public static CandidateApngFrames decode(byte[] bytes) throws IOException {
        if (bytes.length < 8 || !Arrays.equals(Arrays.copyOf(bytes, 8), SIGNATURE)) return null;
        if (bytes.length > 1_048_576) throw invalid();
        ByteBuffer input = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN);
        input.position(8);
        byte[] header = null;
        int width = 0, height = 0, count = 0, sequence = 0;
        long loops = 0;
        boolean imageData = false, ended = false;
        ArrayList<byte[]> shared = new ArrayList<>();
        ArrayList<Part> parts = new ArrayList<>();
        Part current = null;
        while (input.remaining() >= 12) {
            int length = input.getInt();
            if (length < 0 || length > input.remaining() - 8) throw invalid();
            int typeStart = input.position();
            byte[] typeBytes = new byte[4];
            input.get(typeBytes);
            String type = new String(typeBytes, StandardCharsets.US_ASCII);
            byte[] data = new byte[length];
            input.get(data);
            long crc = Integer.toUnsignedLong(input.getInt());
            CRC32 check = new CRC32();
            check.update(bytes, typeStart, length + 4);
            if (crc != check.getValue()) throw invalid();
            ByteBuffer value = ByteBuffer.wrap(data).order(ByteOrder.BIG_ENDIAN);
            switch (type) {
                case "IHDR" -> {
                    if (header != null || typeStart != 12 || length != 13) throw invalid();
                    header = data;
                    width = value.getInt(); height = value.getInt();
                    if (width <= 0 || height <= 0 || width > 2048 || height > 2048) throw invalid();
                }
                case "acTL" -> {
                    if (header == null || imageData || count != 0 || length != 8) throw invalid();
                    count = value.getInt(); loops = Integer.toUnsignedLong(value.getInt());
                    if (count <= 0 || count > 120 || (long) width * height * count > 8_000_000) throw invalid();
                }
                case "fcTL" -> {
                    if (count == 0 || length != 26 || value.getInt() != sequence++) throw invalid();
                    if (current != null && current.data.size() == 0) throw invalid();
                    current = new Part();
                    current.width = value.getInt(); current.height = value.getInt();
                    current.x = value.getInt(); current.y = value.getInt();
                    int numerator = Short.toUnsignedInt(value.getShort());
                    int denominator = Short.toUnsignedInt(value.getShort());
                    long delay = Math.round(1000.0 * numerator / (denominator == 0 ? 100 : denominator));
                    current.delay = delay > 10 ? delay : 100;
                    current.dispose = Byte.toUnsignedInt(value.get());
                    current.blend = Byte.toUnsignedInt(value.get());
                    if (current.width <= 0 || current.height <= 0 || current.x < 0 || current.y < 0
                            || (long) current.x + current.width > width
                            || (long) current.y + current.height > height
                            || current.dispose > 2 || current.blend > 1 || parts.size() >= count) throw invalid();
                    if (!imageData && (current.width != width || current.height != height
                            || current.x != 0 || current.y != 0)) throw invalid();
                    parts.add(current);
                }
                case "IDAT" -> {
                    imageData = true;
                    if (current != null) current.data.write(data);
                }
                case "fdAT" -> {
                    if (!imageData || current == null || length < 4 || value.getInt() != sequence++) throw invalid();
                    current.data.write(data, 4, length - 4);
                }
                case "IEND" -> {
                    if (length != 0 || input.hasRemaining()) throw invalid();
                    ended = true;
                }
                default -> {
                    // 保留调色板、透明度和色彩空间；不把动画控制块塞进重建的静态 PNG。
                    if (!imageData) shared.add(Arrays.copyOfRange(bytes, typeStart - 4, input.position()));
                }
            }
        }
        if (count == 0) return null;
        if (!ended || parts.size() != count || current == null || current.data.size() == 0) throw invalid();
        Bitmap working = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
        ArrayList<Bitmap> decoded = new ArrayList<>();
        long[] ends = new long[count];
        Canvas canvas = new Canvas(working);
        Paint clear = new Paint();
        clear.setXfermode(new PorterDuffXfermode(PorterDuff.Mode.CLEAR));
        try {
            for (int i = 0; i < count; i++) {
                Part part = parts.get(i);
                byte[] subHeader = header.clone();
                ByteBuffer.wrap(subHeader).order(ByteOrder.BIG_ENDIAN).putInt(part.width).putInt(part.height);
                ByteArrayOutputStream png = new ByteArrayOutputStream();
                png.write(SIGNATURE);
                writeChunk(png, "IHDR", subHeader);
                for (byte[] chunk : shared) png.write(chunk);
                writeChunk(png, "IDAT", part.data.toByteArray());
                writeChunk(png, "IEND", new byte[0]);
                byte[] encoded = png.toByteArray();
                Bitmap tile = BitmapFactory.decodeByteArray(encoded, 0, encoded.length);
                if (tile == null || tile.getWidth() != part.width || tile.getHeight() != part.height) throw invalid();
                Bitmap previous = part.dispose == 2 && i != 0 ? working.copy(Bitmap.Config.ARGB_8888, true) : null;
                try {
                    if (part.blend == 0) clear(canvas, clear, part);
                    canvas.drawBitmap(tile, part.x, part.y, null);
                    decoded.add(working.copy(Bitmap.Config.ARGB_8888, false));
                    ends[i] = (i == 0 ? 0 : ends[i - 1]) + part.delay;
                    if (part.dispose == 1 || (part.dispose == 2 && i == 0)) clear(canvas, clear, part);
                    else if (previous != null) {
                        working.eraseColor(0);
                        canvas.drawBitmap(previous, 0, 0, null);
                    }
                } finally {
                    tile.recycle();
                    if (previous != null) previous.recycle();
                }
            }
            return new CandidateApngFrames(decoded.toArray(new Bitmap[0]), ends, loops);
        } catch (IOException | RuntimeException error) {
            for (Bitmap bitmap : decoded) bitmap.recycle();
            throw error;
        } finally { working.recycle(); }
    }

    private static void clear(Canvas canvas, Paint clear, Part part) {
        canvas.drawRect(part.x, part.y, part.x + part.width, part.y + part.height, clear);
    }

    private static void writeChunk(ByteArrayOutputStream output, String type, byte[] data) throws IOException {
        DataOutputStream writer = new DataOutputStream(output);
        byte[] name = type.getBytes(StandardCharsets.US_ASCII);
        writer.writeInt(data.length); writer.write(name); writer.write(data);
        CRC32 crc = new CRC32(); crc.update(name); crc.update(data);
        writer.writeInt((int) crc.getValue());
    }

    private static IOException invalid() { return new IOException("invalid candidate APNG"); }
    private static final class Part {
        int width, height, x, y, dispose, blend;
        long delay;
        final ByteArrayOutputStream data = new ByteArrayOutputStream();
    }
}
