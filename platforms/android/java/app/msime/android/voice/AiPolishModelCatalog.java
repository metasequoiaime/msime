package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.URISyntaxException;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** Native, bounded model-directory access for the Android settings surface. */
public final class AiPolishModelCatalog {
    private static final int MAX_MODELS = 5_000;
    private static final int MAX_PAGES = 10;
    private static final int MAX_MODEL_ID_LENGTH = 256;

    private AiPolishModelCatalog() {}

    public static List<String> fetch(String endpoint, String token) throws AiPolishClient.Failure {
        final AiPolishConfiguration configuration;
        try {
            configuration = new AiPolishConfiguration(
                endpoint, "model-catalog", AiPolishConfiguration.DEFAULT_PROMPT, token);
        } catch (IllegalArgumentException error) {
            throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID, error);
        }
        if (configuration.token().isEmpty())
            throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID);

        URI base = modelsUri(configuration.endpoint());
        boolean anthropic = "api.anthropic.com".equalsIgnoreCase(base.getHost());
        Set<String> models = new TreeSet<>();
        String cursor = null;
        Set<String> cursors = new TreeSet<>();
        for (int page = 0; page < MAX_PAGES; page++) {
            URI requestUri = base;
            if (anthropic) {
                requestUri = withAnthropicQuery(base, cursor);
            }
            JSONObject document = get(requestUri, configuration.token(), anthropic);
            JSONArray data = document.optJSONArray("data");
            if (data == null) throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID);
            for (int index = 0; index < data.length(); index++) {
                JSONObject model = data.optJSONObject(index);
                if (model == null || (model.has("active") && !model.optBoolean("active", true))) continue;
                String id = model.optString("id", "").trim();
                if (id.isEmpty() || id.length() > MAX_MODEL_ID_LENGTH) continue;
                JSONArray endpointTypes = model.optJSONArray("supported_endpoint_types");
                if (endpointTypes != null && endpointTypes.length() > 0) {
                    boolean supported = model.optBoolean("chat_completions_bridge", false);
                    for (int item = 0; item < endpointTypes.length(); item++) {
                        String type = endpointTypes.optString(item, "");
                        if ("openai".equals(type)) supported = true;
                    }
                    if (!supported) continue;
                }
                models.add(id);
                if (models.size() > MAX_MODELS)
                    throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID);
            }
            if (!document.optBoolean("has_more", false)) {
                if (models.isEmpty()) throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID);
                return new ArrayList<>(models);
            }
            if (!anthropic) throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID);
            String next = document.optString("last_id", "").trim();
            if (next.isEmpty() || !cursors.add(next))
                throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID);
            cursor = next;
        }
        throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID);
    }

    private static JSONObject get(URI uri, String token, boolean anthropic)
            throws AiPolishClient.Failure {
        HttpsURLConnection connection = null;
        try {
            connection = (HttpsURLConnection) uri.toURL().openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("GET");
            connection.setConnectTimeout(30_000);
            connection.setReadTimeout(30_000);
            connection.setRequestProperty("Accept", "application/json");
            if (anthropic) {
                connection.setRequestProperty("x-api-key", token);
                connection.setRequestProperty("anthropic-version", "2023-06-01");
            } else connection.setRequestProperty("Authorization", "Bearer " + token);
            int status = connection.getResponseCode();
            if (status < 200 || status >= 300)
                throw new AiPolishClient.Failure(AiPolishClient.Reason.UNAVAILABLE);
            byte[] response;
            try (InputStream input = connection.getInputStream()) {
                response = readBounded(input);
            }
            return new JSONObject(new String(response, StandardCharsets.UTF_8));
        } catch (AiPolishClient.Failure error) {
            throw error;
        } catch (IOException error) {
            throw new AiPolishClient.Failure(AiPolishClient.Reason.UNAVAILABLE, error);
        } catch (JSONException | ClassCastException error) {
            throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID, error);
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    private static byte[] readBounded(InputStream input) throws IOException, AiPolishClient.Failure {
        byte[] response = HttpBodyPolicy.readBounded(
            input, AiPolishConfiguration.MAXIMUM_RESPONSE_BYTES);
        if (response == null) throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID);
        return response;
    }

    static URI modelsUri(URI endpoint) throws AiPolishClient.Failure {
        String path = endpoint.getRawPath() == null ? "" : endpoint.getRawPath();
        while (path.endsWith("/")) path = path.substring(0, path.length() - 1);
        String chatSuffix = "/chat/completions";
        String transcriptionSuffix = "/audio/transcriptions";
        if (path.endsWith(chatSuffix)) path = path.substring(0, path.length() - chatSuffix.length());
        if (path.endsWith(transcriptionSuffix)) {
            path = path.substring(0, path.length() - transcriptionSuffix.length());
        }
        if (!path.endsWith("/")) path += "/";
        return rawUri(endpoint, path + "models", endpoint.getRawQuery());
    }

    static URI withQuery(URI base, String query) throws AiPolishClient.Failure {
        return rawUri(base, base.getRawPath(), query);
    }

    static URI withAnthropicQuery(URI base, String cursor)
            throws AiPolishClient.Failure {
        StringBuilder query = new StringBuilder();
        appendPreservedQuery(base.getRawQuery(), query);
        appendQueryPart(query, "limit=1000");
        if (cursor != null) appendQueryPart(query, "after_id=" + encode(cursor));
        return withQuery(base, query.toString());
    }

    private static void appendPreservedQuery(String rawQuery, StringBuilder output) {
        if (rawQuery == null || rawQuery.isEmpty()) return;
        for (String part : rawQuery.split("&", -1)) {
            if (part.isEmpty()) continue;
            int equals = part.indexOf('=');
            String rawName = equals < 0 ? part : part.substring(0, equals);
            String name;
            try {
                name = java.net.URLDecoder.decode(rawName, StandardCharsets.UTF_8.name());
            } catch (IllegalArgumentException | java.io.UnsupportedEncodingException error) {
                name = rawName;
            }
            if ("limit".equals(name) || "after_id".equals(name)) continue;
            appendQueryPart(output, part);
        }
    }

    private static void appendQueryPart(StringBuilder query, String part) {
        if (query.length() > 0) query.append('&');
        query.append(part);
    }

    private static URI rawUri(URI base, String path, String rawQuery)
            throws AiPolishClient.Failure {
        try {
            StringBuilder value = new StringBuilder(base.getScheme()).append("://")
                .append(base.getRawAuthority());
            if (path == null || path.isEmpty()) value.append('/');
            else if (path.charAt(0) == '/') value.append(path);
            else value.append('/').append(path);
            if (rawQuery != null && !rawQuery.isEmpty()) value.append('?').append(rawQuery);
            return new URI(value.toString());
        } catch (URISyntaxException error) {
            throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID, error);
        }
    }

    private static String encode(String value) {
        try {
            // The Charset overload is API 33; the name overload has been there since API 1 and
            // throws a checked exception that UTF-8 cannot actually raise.
            return URLEncoder.encode(value, StandardCharsets.UTF_8.name());
        } catch (java.io.UnsupportedEncodingException error) {
            throw new IllegalStateException("UTF-8 is always supported", error);
        }
    }
}
