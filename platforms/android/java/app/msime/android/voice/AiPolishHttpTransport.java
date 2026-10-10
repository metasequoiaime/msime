package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URI;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 有界的 Chat Completions 传输：https，或 AiEndpointPolicy 放行的本机、局域网 http。不跟随重定向，也不暴露响应正文。 */
public final class AiPolishHttpTransport implements AiPolishClient.Transport {
    @Override public String send(AiPolishConfiguration configuration, String text,
                                 AiPolishClient.Cancellation cancellation)
            throws AiPolishClient.Failure {
        HttpURLConnection connection = null;
        try {
            JSONObject body = new JSONObject().put("model", configuration.model())
                .put("messages", new JSONArray()
                    .put(new JSONObject().put("role", "system").put("content", configuration.prompt()))
                    .put(new JSONObject().put("role", "user").put("content", text)));
            byte[] bytes = TextPolicy.utf8Bytes(body.toString());
            connection = (HttpURLConnection) AiEndpointPolicy.open(configuration.endpoint().toURL());
            HttpURLConnection target = connection;
            cancellation.attach(target::disconnect);
            if (cancellation.cancelled()) throw new AiPolishClient.Failure(AiPolishClient.Reason.CANCELLED);
            HttpConnectionPolicy.rejectRedirects(connection);
            connection.setRequestMethod("POST");
            connection.setConnectTimeout(60_000);
            connection.setReadTimeout(60_000);
            connection.setDoOutput(true);
            connection.setFixedLengthStreamingMode(bytes.length);
            connection.setRequestProperty("Content-Type", "application/json");
            connection.setRequestProperty("Accept", "application/json");
            for (Map.Entry<String, String> header : authenticationHeaders(
                    configuration.endpoint(), configuration.token()).entrySet()) {
                connection.setRequestProperty(header.getKey(), header.getValue());
            }
            try (OutputStream output = connection.getOutputStream()) { output.write(bytes); }
            int status = connection.getResponseCode();
            if (status < 200 || status >= 300)
                throw new AiPolishClient.Failure(AiPolishClient.Reason.UNAVAILABLE);
            byte[] response;
            try (InputStream input = connection.getInputStream()) {
                response = HttpBodyPolicy.readBounded(input,
                    AiPolishConfiguration.MAXIMUM_RESPONSE_BYTES, cancellation::cancelled);
                if (cancellation.cancelled())
                    throw new AiPolishClient.Failure(AiPolishClient.Reason.CANCELLED);
                if (response == null) throw new AiPolishClient.Failure(AiPolishClient.Reason.INVALID);
            }
            JSONObject document = new JSONObject(TextPolicy.utf8(response));
            Object content = document.getJSONArray("choices").getJSONObject(0)
                .getJSONObject("message").opt("content");
            return JsonPolicy.strictStringOrEmpty(content);
        } catch (AiPolishClient.Failure error) {
            throw error;
        } catch (IOException | JSONException | ClassCastException | SecurityException error) {
            if (cancellation.cancelled())
                throw new AiPolishClient.Failure(AiPolishClient.Reason.CANCELLED, error);
            throw new AiPolishClient.Failure(AiPolishClient.Reason.UNAVAILABLE, error);
        } finally {
            cancellation.detach();
            if (connection != null) connection.disconnect();
        }
    }

    static Map<String, String> authenticationHeaders(URI endpoint, String token) {
        return AiProviderHeaders.forHost(endpoint.getHost(), token);
    }

    /** Chat completions carry text; do not let org.json coerce malformed values into prose. */
}
