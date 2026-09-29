package app.msime.android;

import android.content.Context;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Locale;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONArray;
import org.json.JSONObject;

/** Small account-backed client for the shared candidate translation endpoint. */
public final class BackendTranslationClient implements CandidateTranslationStore.Service {
    private static final String ORIGIN = "https://api.msime.app";
    private static final int MAX_RESPONSE_BYTES = 256 * 1024;
    private final AndroidAccountSessionStorage storage;
    private final BackendAnonymousAccount anonymous;

    public BackendTranslationClient(Context context) {
        storage = new AndroidAccountSessionStorage(context);
        anonymous = new BackendAnonymousAccount(context);
    }

    @Override public List<String> translate(List<String> texts, String target) throws Exception {
        if (texts == null || texts.isEmpty() || texts.size() > 32
                || target == null || target.isEmpty() || TextPolicy.utf8Length(target) > 16)
            throw new IllegalArgumentException("Invalid translation request");
        for (String text : texts) {
            if (text == null || text.isEmpty() || TextPolicy.utf8Length(text) > 2048)
                throw new IllegalArgumentException("Invalid translation text");
        }
        String token = accessToken();
        JSONObject body = new JSONObject().put("texts", new JSONArray(texts))
            .put("source_lang", "ZH").put("target_lang", target.toUpperCase(Locale.ROOT));
        byte[] request = body.toString().getBytes(StandardCharsets.UTF_8);
        if (request.length > 64 * 1024) throw new IllegalArgumentException("Translation request is too large");
        HttpsURLConnection connection = null;
        try {
            connection = (HttpsURLConnection) new URL(ORIGIN + "/v1/translate").openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("POST");
            connection.setConnectTimeout(30_000);
            connection.setReadTimeout(30_000);
            connection.setDoOutput(true);
            connection.setFixedLengthStreamingMode(request.length);
            connection.setRequestProperty("Authorization", "Bearer " + token);
            connection.setRequestProperty("User-Agent", "MSIME/Android");
            connection.setRequestProperty("Accept", "application/json");
            connection.setRequestProperty("Content-Type", "application/json");
            try (OutputStream output = connection.getOutputStream()) { output.write(request); }
            if (connection.getResponseCode() != 200) throw new IllegalStateException("Translation unavailable");
            byte[] bytes;
            try (InputStream input = connection.getInputStream()) { bytes = readBounded(input); }
            JSONObject response = new JSONObject(new String(bytes, StandardCharsets.UTF_8));
            if (response.optInt("code", -1) != 200) throw new IllegalStateException("Translation failed");
            JSONArray values = response.getJSONArray("data");
            if (values.length() != texts.size()) throw new IllegalStateException("Translation response mismatch");
            java.util.ArrayList<String> result = new java.util.ArrayList<>(values.length());
            for (int index = 0; index < values.length(); index++) {
                String value = values.optString(index, "").trim();
                if (value.isEmpty() || TextPolicy.utf8Length(value) > 4096
                        || value.chars().anyMatch(ch -> ch == '\n' || ch == '\r' || (ch < 0x20 && ch != '\t')))
                    throw new IllegalStateException("Invalid translation response");
                result.add(value);
            }
            return result;
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    private String accessToken() throws Exception {
        String encoded = storage.load();
        if (encoded != null) {
            JSONObject saved = new JSONObject(encoded);
            JSONObject tokens = saved.getJSONObject("tokens");
            String token = tokens.getString("access_token");
            long expiry = saved.optLong("expires_at_unix_ms", Long.MAX_VALUE);
            if (token.matches("[0-9a-fA-F]{64}") && expiry > System.currentTimeMillis() + 30_000L) return token;
        }
        return anonymous.accessToken();
    }

    private static byte[] readBounded(InputStream input) throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (output.size() + count > MAX_RESPONSE_BYTES)
                throw new IllegalStateException("Translation response is too large");
            output.write(buffer, 0, count);
        }
        return output.toByteArray();
    }
}
