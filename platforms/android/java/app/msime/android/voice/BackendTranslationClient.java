package app.msime.android;

import android.content.Context;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.math.BigDecimal;
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
    private final BackendAccount account;
    private final BackendAnonymousAccount anonymous;

    public BackendTranslationClient(Context context) {
        account = new BackendAccount(context);
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
            List<String> result = parseResponse(bytes, texts.size());
            if (result == null) throw new IllegalStateException("Invalid translation response");
            return result;
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    /** Decode the response without allowing org.json to coerce nulls or non-strings to text. */
    static List<String> parseResponse(byte[] bytes, int expectedCount) throws Exception {
        JSONObject response = new JSONObject(new String(bytes, StandardCharsets.UTF_8));
        if (!successStatusCode(response.opt("code"))) return null;
        Object data = response.opt("data");
        if (!(data instanceof JSONArray)) return null;
        JSONArray values = (JSONArray) data;
        java.util.ArrayList<Object> rawValues = new java.util.ArrayList<>(values.length());
        for (int index = 0; index < values.length(); index++) {
            rawValues.add(values.opt(index));
        }
        return parseValues(rawValues, expectedCount);
    }

    /** A successful response status is an integer JSON number, never a coerced fraction or boolean. */
    static boolean successStatusCode(Object value) {
        if (!(value instanceof Number) || value instanceof Boolean) return false;
        if (value instanceof Double || value instanceof Float) return false;
        try {
            return new BigDecimal(value.toString()).intValueExact() == 200;
        } catch (NumberFormatException | ArithmeticException exception) {
            return false;
        }
    }

    /** Validate already-decoded values without org.json's coercing accessors. */
    static List<String> parseValues(List<?> values, int expectedCount) {
        if (values == null || values.size() != expectedCount) return null;
        java.util.ArrayList<String> result = new java.util.ArrayList<>(values.size());
        for (Object raw : values) {
            if (!(raw instanceof String)) return null;
            String value = ((String) raw).trim();
            if (value.isEmpty() || TextPolicy.utf8Length(value) > 4096
                    || value.chars().anyMatch(ch -> ch == '\n' || ch == '\r' || (ch < 0x20 && ch != '\t')))
                return null;
            result.add(value);
        }
        return result;
    }

    private String accessToken() throws Exception {
        String token = account.accessToken();
        return token.isEmpty() ? anonymous.accessToken() : token;
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
