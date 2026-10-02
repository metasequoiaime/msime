package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Base64;

/**
 * The parts of RFC 6455 this host needs to talk to a streaming recogniser.
 *
 * <p>Android has no WebSocket in its platform API, and the streaming ASR protocol is a WebSocket.
 * Rather than take a dependency for one endpoint, the handshake and the frame format are here —
 * both are small, both are fully determined by the RFC, and both are exactly the kind of thing
 * worth pinning with tests rather than trusting to a first reading.
 *
 * <p>Only what that conversation uses: binary frames out, binary frames in, close, and ping/pong
 * so a server keepalive does not drop the connection. Extensions are never negotiated, so no frame
 * this host sends or accepts is compressed, and continuation frames are reassembled rather than
 * rejected because a long recognition result legitimately arrives in pieces.
 */
public final class WebSocketFrames {
    /** The constant RFC 6455 appends to the client key before hashing. */
    private static final String ACCEPT_SUFFIX = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    public static final int OPCODE_CONTINUATION = 0x0;
    public static final int OPCODE_TEXT = 0x1;
    public static final int OPCODE_BINARY = 0x2;
    public static final int OPCODE_CLOSE = 0x8;
    public static final int OPCODE_PING = 0x9;
    public static final int OPCODE_PONG = 0xa;

    private WebSocketFrames() {}

    /** The `Sec-WebSocket-Accept` a server must return for this key. */
    public static String acceptFor(String key) {
        try {
            MessageDigest sha1 = MessageDigest.getInstance("SHA-1");
            byte[] digest = sha1.digest((key + ACCEPT_SUFFIX).getBytes(StandardCharsets.US_ASCII));
            return Base64.getEncoder().encodeToString(digest);
        } catch (NoSuchAlgorithmException error) {
            // SHA-1 is required of every Java platform; there is no recovery and no fallback that
            // would still be a WebSocket handshake.
            throw new IllegalStateException("SHA-1 unavailable", error);
        }
    }

    /**
     * The upgrade request for this endpoint.
     *
     * <p>`key` is the caller's 16 random bytes, base64-encoded. It is not a secret and not
     * authentication — it exists so a cached or confused intermediary cannot pass an old response
     * off as a successful handshake.
     */
    public static String handshakeRequest(String host, String path, String key, String[] headers) {
        StringBuilder request = new StringBuilder()
            .append("GET ").append(path).append(" HTTP/1.1\r\n")
            .append("Host: ").append(host).append("\r\n")
            .append("Upgrade: websocket\r\n")
            .append("Connection: Upgrade\r\n")
            .append("Sec-WebSocket-Key: ").append(key).append("\r\n")
            .append("Sec-WebSocket-Version: 13\r\n");
        if (headers != null) {
            for (int index = 0; index + 1 < headers.length; index += 2) {
                request.append(headers[index]).append(": ").append(headers[index + 1]).append("\r\n");
            }
        }
        return request.append("\r\n").toString();
    }

    /** Whether the response line and headers are a successful upgrade for this key. */
    public static boolean handshakeAccepted(String response, String key) {
        if (response == null || !response.startsWith("HTTP/1.1 101")) return false;
        String expected = acceptFor(key);
        boolean upgrade = false;
        boolean connection = false;
        boolean accepted = false;
        for (String line : response.split("\r\n")) {
            int separator = line.indexOf(':');
            if (separator <= 0) continue;
            String name = line.substring(0, separator).trim().toLowerCase(java.util.Locale.ROOT);
            String value = line.substring(separator + 1).trim();
            switch (name) {
                case "upgrade" -> upgrade = value.equalsIgnoreCase("websocket");
                case "connection" -> connection = value.toLowerCase(java.util.Locale.ROOT)
                    .contains("upgrade");
                case "sec-websocket-accept" -> accepted = value.equals(expected);
                // An extension this host never offered must not be applied to its frames.
                case "sec-websocket-extensions" -> {
                    if (!value.isEmpty()) return false;
                }
                default -> { }
            }
        }
        return upgrade && connection && accepted;
    }

    /**
     * One client frame: always masked, as the RFC requires of a client.
     *
     * <p>`mask` is the caller's four bytes. Masking is not encryption — the transport's TLS is what
     * protects the payload — it exists so a frame's bytes cannot be steered to look like a
     * plaintext HTTP request to an intermediary that is guessing.
     */
    public static byte[] clientFrame(int opcode, byte[] payload, int length, byte[] mask) {
        byte[] body = payload == null ? new byte[0] : payload;
        int size = KeyboardGeometry.bounded(length, 0, body.length);
        int headerLength = 2 + lengthBytes(size) + 4;
        byte[] frame = new byte[headerLength + size];
        frame[0] = (byte) (0x80 | opcode & 0x0f);
        int offset = writeLength(frame, size);
        for (int index = 0; index < 4; index++) frame[offset + index] = mask[index];
        offset += 4;
        for (int index = 0; index < size; index++) {
            frame[offset + index] = (byte) (body[index] ^ mask[index % 4]);
        }
        return frame;
    }

    /** One decoded frame, or null when the buffer does not yet hold a whole one. */
    public record Frame(boolean fin, int opcode, byte[] payload, int consumed) {}

    /**
     * Decode the frame at the start of `buffer`, or null when more bytes are needed.
     *
     * <p>A server frame is never masked; one that claims to be is a protocol error rather than
     * something to unmask, and is refused.
     */
    public static Frame decode(byte[] buffer, int available) {
        if (buffer == null || available < 2) return null;
        boolean fin = (buffer[0] & 0x80) != 0;
        int opcode = buffer[0] & 0x0f;
        if ((buffer[0] & 0x70) != 0) return null;
        if ((buffer[1] & 0x80) != 0) return null;
        long length = buffer[1] & 0x7f;
        int offset = 2;
        if (length == 126) {
            if (available < 4) return null;
            length = (buffer[2] & 0xffL) << 8 | buffer[3] & 0xffL;
            offset = 4;
        } else if (length == 127) {
            if (available < 10) return null;
            length = 0;
            for (int index = 0; index < 8; index++) {
                length = length << 8 | buffer[2 + index] & 0xffL;
            }
            offset = 10;
        }
        // A frame this host could not hold is refused rather than allocated: the recogniser sends
        // transcripts, and anything of this size is a wrong endpoint or a hostile one.
        if (length < 0 || length > 8L * 1024 * 1024
                || ((opcode & 0x8) != 0 && (!fin || length > 125))) return null;
        if (available < offset + length) return null;
        byte[] payload = new byte[(int) length];
        System.arraycopy(buffer, offset, payload, 0, (int) length);
        return new Frame(fin, opcode, payload, offset + (int) length);
    }

    private static int lengthBytes(int size) {
        if (size < 126) return 0;
        return size <= 0xffff ? 2 : 8;
    }

    private static int writeLength(byte[] frame, int size) {
        if (size < 126) {
            frame[1] = (byte) (0x80 | size);
            return 2;
        }
        if (size <= 0xffff) {
            frame[1] = (byte) (0x80 | 126);
            frame[2] = (byte) (size >>> 8 & 0xff);
            frame[3] = (byte) (size & 0xff);
            return 4;
        }
        frame[1] = (byte) (0x80 | 127);
        // Widen first: an int shift distance is taken mod 32.
        long value = size;
        for (int index = 0; index < 8; index++) {
            frame[2 + index] = (byte) (value >>> (7 - index) * 8 & 0xff);
        }
        return 10;
    }
}
