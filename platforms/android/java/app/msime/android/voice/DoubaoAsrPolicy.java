package app.msime.android;

import java.util.List;

/**
 * Whether a request is the streaming protocol, and whether this host can run it.
 *
 * <p>Doubao is the one provider that streams: the audio leaves while the user is still speaking and
 * the transcript comes back in pieces. It is also the one that authenticates with headers rather
 * than a bearer token, which is why the checks here are about headers and a `wss://` endpoint
 * rather than a model and a key.
 *
 * <p>The headers themselves are built by the shared authentication policy before they reach this
 * host — both auth modes, and which names each one produces. What this checks is that what arrived
 * is the set that protocol needs, so a half-configured account fails here rather than at a socket.
 */
public final class DoubaoAsrPolicy {
    public static final String PROVIDER = "doubao";
    /** Every mode sends the resource id and a request id; the credential pair varies by mode. */
    private static final String RESOURCE_HEADER = "x-api-resource-id";
    private static final String REQUEST_HEADER = "x-api-request-id";

    private DoubaoAsrPolicy() {}

    public static boolean isStreaming(String provider) {
        return PROVIDER.equals(provider);
    }

    /** A streaming response carries text; reject non-string JSON values before display. */
    static String strictText(Object value) {
        return value instanceof String ? (String) value : "";
    }

    static boolean validEndpoint(String endpoint) {
        return TextPolicy.validAuthority(endpoint, "wss://", 2048);
    }

    /**
     * Whether this host can open the session as configured.
     *
     * <p>`wss://` only: the credentials travel in the handshake's own headers, so a downgrade to
     * plaintext would put them on the wire, and this host is the one opening the connection.
     */
    public static boolean usable(String provider, String endpoint, List<String> headerNames) {
        if (!isStreaming(provider)) return false;
        if (!validEndpoint(endpoint)) {
            return false;
        }
        if (headerNames == null || headerNames.size() < 3 || headerNames.size() > 8) return false;
        boolean resource = false;
        boolean request = false;
        boolean credential = false;
        for (String name : headerNames) {
            if (name == null || name.isEmpty() || TextPolicy.hasControl(name)) return false;
            switch (name) {
                case RESOURCE_HEADER -> resource = true;
                case REQUEST_HEADER -> request = true;
                // Either the single API key or the app-key/access-key pair, depending on the mode
                // the shared policy resolved; one of them being present is what makes it usable.
                case "x-api-key", "x-api-access-key" -> credential = true;
                case "x-api-app-key" -> { }
                default -> { return false; }
            }
        }
        return resource && request && credential;
    }

}
