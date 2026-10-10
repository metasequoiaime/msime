package app.msime.android;

import android.content.Context;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.math.BigDecimal;
import java.net.URL;
import java.util.List;
import java.util.Map;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONArray;
import org.json.JSONObject;

/** Small account-backed client for the shared candidate translation endpoint. */
public final class BackendTranslationClient implements CandidateTranslationStore.Service {
    /** Maximum number of source texts accepted by one translation request. */
    public static final int MAX_TEXTS = 32;
    private static final int MAX_RESPONSE_BYTES = 256 * 1024;
    private final CloudApi cloud;

    public BackendTranslationClient(Context context) {
        this(translationCloud(context.getApplicationContext()));
    }

    BackendTranslationClient(CloudApi cloud) {
        this.cloud = cloud;
    }

    private static CloudApi translationCloud(Context application) {
        return new CloudApi(BackendTranslationClient::httpExchange,
            CloudApi.accountTokens(application),
            rejected -> new BackendAnonymousAccount(application).accessToken(rejected));
    }

    /** Keep the translation endpoint's 256 KiB wire bound while CloudApi fences its login. */
    private static CloudApi.Exchange httpExchange(String method, String path,
            Map<String, String> headers, byte[] request) throws IOException {
        HttpsURLConnection connection = (HttpsURLConnection) new URL(CloudApi.ORIGIN + path).openConnection();
        try {
            HttpConnectionPolicy.rejectRedirects(connection);
            connection.setRequestMethod(method);
            connection.setConnectTimeout(30_000);
            connection.setReadTimeout(30_000);
            for (Map.Entry<String, String> header : headers.entrySet())
                connection.setRequestProperty(header.getKey(), header.getValue());
            if (request != null) {
                connection.setDoOutput(true);
                connection.setFixedLengthStreamingMode(request.length);
                try (OutputStream output = connection.getOutputStream()) { output.write(request); }
            }
            int status = connection.getResponseCode();
            InputStream stream = status / 100 == 2 ? connection.getInputStream() : connection.getErrorStream();
            byte[] response = new byte[0];
            if (stream != null) {
                try (InputStream input = stream) {
                    if (status / 100 == 2) {
                        response = HttpBodyPolicy.readRequired(input, MAX_RESPONSE_BYTES);
                    } else {
                        try { response = HttpBodyPolicy.readRequired(input, MAX_RESPONSE_BYTES); }
                        catch (IOException unreadable) { response = new byte[0]; }
                    }
                }
            }
            return new CloudApi.Exchange(status, connection.getContentType(),
                connection.getHeaderField("Retry-After"), response);
        } finally {
            connection.disconnect();
        }
    }

    @Override public List<String> translate(List<String> texts, String target) throws Exception {
        if (texts == null || texts.isEmpty() || texts.size() > MAX_TEXTS
                || target == null || target.isEmpty() || TextPolicy.utf8Length(target) > 16)
            throw new IllegalArgumentException("Invalid translation request");
        for (String text : texts) {
            if (text == null || text.isEmpty() || TextPolicy.utf8Length(text) > 2048)
                throw new IllegalArgumentException("Invalid translation text");
        }
        JSONObject body = new JSONObject().put("texts", new JSONArray(texts))
            .put("source_lang", "ZH").put("target_lang", TextPolicy.uppercase(target));
        byte[] request = TextPolicy.utf8Bytes(body.toString());
        if (request.length > 64 * 1024) throw new IllegalArgumentException("Translation request is too large");
        CloudApi.Response response = cloud.send("POST", "/v1/translate",
            new CloudApi.Body("application/json", request), CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
        if (response.status() != 200) throw new BackendAccount.RequestException(response.status());
        byte[] bytes = response.body();
        if (bytes == null || bytes.length > MAX_RESPONSE_BYTES)
            throw new IllegalStateException("Translation response is too large");
        List<String> result = parseResponse(bytes, texts.size());
        if (result == null) throw new IllegalStateException("Invalid translation response");
        return result;
    }

    /** Decode the response without allowing org.json to coerce nulls or non-strings to text. */
    static List<String> parseResponse(byte[] bytes, int expectedCount) throws Exception {
        JSONObject response = new JSONObject(TextPolicy.utf8(bytes));
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
            String value = TextPolicy.trimmed((String) raw);
            if (value.isEmpty() || TextPolicy.utf8Length(value) > 4096
                    || value.chars().anyMatch(ch -> ch == '\n' || ch == '\r' || (ch < 0x20 && ch != '\t')))
                return null;
            result.add(value);
        }
        return result;
    }

}
