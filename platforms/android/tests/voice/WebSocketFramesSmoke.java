import app.msime.android.WebSocketFrames;
import app.msime.android.WebSocketFrames.Frame;
import java.nio.charset.StandardCharsets;

/** RFC 6455, only the parts the streaming recogniser uses, pinned against the RFC's own example. */
public final class WebSocketFramesSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static byte[] concat(byte[] first, byte[] second) {
        byte[] out = new byte[first.length + second.length];
        System.arraycopy(first, 0, out, 0, first.length);
        System.arraycopy(second, 0, out, first.length, second.length);
        return out;
    }

    public static void main(String[] args) {
        // The worked example from RFC 6455 section 1.3. Getting this wrong means every handshake
        // is accepted or every handshake is refused, and only one of those is noticeable.
        check("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=".equals(WebSocketFrames.acceptFor("dGhlIHNhbXBsZSBub25jZQ==")),
            "the accept value matches the RFC's own example");

        String key = "AQIDBAUGBwgJCgsMDQ4PEC==";
        String request = WebSocketFrames.handshakeRequest("example.invalid", "/v3/sauc", key,
            new String[] {"X-Api-Key", "secret"});
        check(request.startsWith("GET /v3/sauc HTTP/1.1\r\n"), "the path is requested");
        check(request.contains("Host: example.invalid\r\n"), "the host is named");
        check(request.contains("Upgrade: websocket\r\n") && request.contains("Connection: Upgrade\r\n"),
            "the upgrade is asked for");
        check(request.contains("Sec-WebSocket-Version: 13\r\n"), "version 13 is the only one");
        check(request.contains("Sec-WebSocket-Key: " + key + "\r\n"), "the key is sent");
        check(request.contains("X-Api-Key: secret\r\n"), "caller headers are carried");
        check(request.endsWith("\r\n\r\n"), "the request is terminated");
        String injected = WebSocketFrames.handshakeRequest("example.invalid", "/v3/sauc", key,
            new String[] {"X-Api-Key", "secret\r\nX-Injected: yes"});
        check(!injected.contains("X-Injected: yes"),
            "header values cannot inject additional handshake lines");
        String injectedName = WebSocketFrames.handshakeRequest("example.invalid", "/v3/sauc", key,
            new String[] {"X-Api-Key\r\nX-Injected", "secret"});
        check(!injectedName.contains("X-Injected"),
            "header names cannot inject additional handshake lines");
        String unicode = WebSocketFrames.handshakeRequest("example.invalid", "/v3/sauc", key,
            new String[] {"X-Api-Key", "密钥"});
        check(!unicode.contains("X-Api-Key:"),
            "non-ASCII header values are refused before US-ASCII encoding corrupts credentials");
        // This host never offers an extension, so it must not advertise one either.
        check(!request.contains("Sec-WebSocket-Extensions"), "no extension is offered");

        String accept = WebSocketFrames.acceptFor(key);
        String ok = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n"
            + "Connection: Upgrade\r\nSec-WebSocket-Accept: " + accept + "\r\n\r\n";
        check(WebSocketFrames.handshakeAccepted(ok, key), "a correct handshake is accepted");
        check(!WebSocketFrames.handshakeAccepted(ok.replace("101", "200"), key),
            "only 101 is an upgrade");
        check(!WebSocketFrames.handshakeAccepted(ok, "AnotherKeyEntirely=="),
            "an accept value for a different key is refused");
        check(!WebSocketFrames.handshakeAccepted(ok.replace("Upgrade: websocket", "Upgrade: h2c"), key),
            "an upgrade to something else is refused");
        check(!WebSocketFrames.handshakeAccepted(ok.replace("Connection: Upgrade", "Connection: notupgrade"), key),
            "Connection must contain the Upgrade token, not merely its letters");
        check(WebSocketFrames.handshakeAccepted(ok.replace("Connection: Upgrade", "Connection: keep-alive, UpGrAdE"), key),
            "Connection may list the Upgrade token with other options");
        check(!WebSocketFrames.handshakeAccepted(ok.substring(0, ok.length() - 2), key),
            "a truncated header block is not an upgrade");
        check(!WebSocketFrames.handshakeAccepted(ok.replace("HTTP/1.1 101 Switching", "HTTP/1.1 101x Switching"), key),
            "a status line without the required separator is refused");
        check(!WebSocketFrames.handshakeAccepted(
                ok.replace("\r\n\r\n", "\r\nSec-WebSocket-Extensions: permessage-deflate\r\n\r\n"), key),
            "an extension this host never offered is refused rather than ignored");
        check(!WebSocketFrames.handshakeAccepted(null, key), "no response is not an upgrade");

        byte[] mask = {0x37, (byte) 0xfa, 0x21, 0x3d};
        byte[] payload = "Hello".getBytes(StandardCharsets.UTF_8);
        byte[] frame = WebSocketFrames.clientFrame(WebSocketFrames.OPCODE_BINARY, payload,
            payload.length, mask);
        check((frame[0] & 0xff) == 0x82, "a single binary frame is FIN with opcode 2");
        check((frame[1] & 0x80) != 0, "a client frame is always masked");
        check((frame[1] & 0x7f) == payload.length, "a short payload carries its own length");
        for (int index = 0; index < payload.length; index++) {
            check(frame[6 + index] == (byte) (payload[index] ^ mask[index % 4]),
                "each payload byte is masked with the rotating key");
        }

        // The two extended length forms, because getting the boundary wrong is silent until a
        // transcript happens to be exactly the wrong size.
        byte[] medium = WebSocketFrames.clientFrame(WebSocketFrames.OPCODE_BINARY,
            new byte[200], 200, mask);
        check((medium[1] & 0x7f) == 126 && (medium[2] & 0xff) == 0 && (medium[3] & 0xff) == 200,
            "126 introduces a two-byte length");
        byte[] boundary = WebSocketFrames.clientFrame(WebSocketFrames.OPCODE_BINARY,
            new byte[125], 125, mask);
        check((boundary[1] & 0x7f) == 125, "125 still fits in the short form");
        byte[] large = WebSocketFrames.clientFrame(WebSocketFrames.OPCODE_BINARY,
            new byte[70000], 70000, mask);
        check((large[1] & 0x7f) == 127, "beyond 65535 introduces an eight-byte length");
        check(large[2] == 0 && large[3] == 0 && large[4] == 0 && large[5] == 0,
            "the high half of the eight-byte length is zero");
        check(large[6] == 0 && large[7] == 0x01 && large[8] == 0x11 && large[9] == 0x70,
            "the low half of the eight-byte length is the size");

        // Server frames are never masked, and decode has to agree with encode on every length form.
        byte[] server = {(byte) 0x82, 0x03, 1, 2, 3};
        Frame decoded = WebSocketFrames.decode(server, server.length);
        check(decoded != null && decoded.fin() && decoded.opcode() == WebSocketFrames.OPCODE_BINARY,
            "a server binary frame decodes");
        check(decoded.payload().length == 3 && decoded.payload()[2] == 3, "the payload survives");
        check(decoded.consumed() == 5, "the frame reports how much of the buffer it used");

        check(WebSocketFrames.decode(server, 1) == null,
            "a partial header waits for more bytes rather than guessing");
        check(WebSocketFrames.decode(server, 4) == null,
            "a partial payload waits too");
        check(WebSocketFrames.decode(server, server.length + 1) == null,
            "a claimed byte count beyond the buffer is refused");
        byte[] masked = {(byte) 0x82, (byte) 0x83, 0, 0, 0, 0, 1, 2, 3};
        check(WebSocketFrames.decode(masked, masked.length) == null,
            "a server frame claiming to be masked is a protocol error, not something to unmask");
        byte[] reserved = {(byte) 0xc2, 0};
        check(WebSocketFrames.decode(reserved, reserved.length) == null,
            "a frame with an RSV bit is refused when no extension was negotiated");
        byte[] fragmentedPing = {0x09, 0};
        check(WebSocketFrames.decode(fragmentedPing, fragmentedPing.length) == null,
            "a control frame must not be fragmented");
        byte[] oversizedPing = {(byte) 0x89, 126, 0, 126};
        check(WebSocketFrames.decode(oversizedPing, oversizedPing.length) == null,
            "a control frame must fit in 125 payload bytes");
        byte[] huge = {(byte) 0x82, 127, 0x7f, -1, -1, -1, -1, -1, -1, -1};
        check(WebSocketFrames.decode(huge, huge.length) == null,
            "a frame this host could not hold is refused rather than allocated");
        byte[] reservedOpcode = {(byte) 0x83, 0};
        check(WebSocketFrames.decode(reservedOpcode, reservedOpcode.length) == null,
            "a reserved data opcode is refused");
        byte[] reservedControlOpcode = {(byte) 0x8b, 0};
        check(WebSocketFrames.decode(reservedControlOpcode, reservedControlOpcode.length) == null,
            "a reserved control opcode is refused");

        // Continuation: a long transcript legitimately arrives in pieces.
        byte[] first = {0x02, 0x02, 1, 2};
        byte[] rest = {(byte) 0x80, 0x01, 3};
        byte[] both = concat(first, rest);
        Frame head = WebSocketFrames.decode(both, both.length);
        check(head != null && !head.fin() && head.opcode() == WebSocketFrames.OPCODE_BINARY,
            "the first fragment is not final");
        Frame tail = WebSocketFrames.decode(java.util.Arrays.copyOfRange(both, head.consumed(),
            both.length), both.length - head.consumed());
        check(tail != null && tail.fin() && tail.opcode() == WebSocketFrames.OPCODE_CONTINUATION,
            "the last fragment is a final continuation");
        System.out.println("Android WebSocket frames passed");
    }
}
