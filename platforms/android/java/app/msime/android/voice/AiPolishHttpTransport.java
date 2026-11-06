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
            JSONObject body = requestBody(configuration.endpoint(), configuration.model(),
                configuration.prompt(), text);
            byte[] bytes = TextPolicy.utf8Bytes(body.toString());
            connection = (HttpURLConnection) AiEndpointPolicy.open(configuration.endpoint().toURL());
            HttpURLConnection target = connection;
            cancellation.attach(target::disconnect);
            if (cancellation.cancelled()) throw new AiPolishClient.Failure(AiPolishClient.Reason.CANCELLED);
            HttpConnectionPolicy.rejectRedirects(connection);
            connection.setRequestMethod("POST");
            HttpConnectionPolicy.setTimeouts(connection, 60_000, 60_000);
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
            if (!HttpConnectionPolicy.isSuccessful(status))
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

    static JSONObject requestBody(URI endpoint, String model, String prompt, String text)
            throws JSONException {
        JSONObject body = new JSONObject().put("model", model)
            .put("messages", new JSONArray()
                .put(new JSONObject().put("role", "system").put("content", prompt))
                .put(new JSONObject().put("role", "user").put("content", text)));
        if (disablesArkSeedThinking(endpoint, model))
            body.put("thinking", new JSONObject().put("type", "disabled"));
        return body;
    }

    /**
     * 火山方舟上的豆包 Seed 默认开着深度思考，润色要显式发 thinking.type = disabled。方舟深度思考文档（https://docs.volcengine.com/docs/82379/1449737 ，2026-11-05 查）列出的 doubao-seed-* 全部支持 disabled，同平台的别的模型不一定（glm-5-3-flash-260828 只支持 enabled），所以接口地址属于 volces.com 且模型属于这个系列才算。与 client-providers 的 disables_ark_seed_thinking 一致。
     */
    static boolean disablesArkSeedThinking(URI endpoint, String model) {
        String host = endpoint.getHost();
        if (host == null || model == null) return false;
        host = host.toLowerCase(java.util.Locale.ROOT);
        return (host.equals("volces.com") || host.endsWith(".volces.com"))
            && model.trim().toLowerCase(java.util.Locale.ROOT).startsWith("doubao-seed-");
    }

    static Map<String, String> authenticationHeaders(URI endpoint, String token) {
        return AiProviderHeaders.forHost(endpoint.getHost(), token);
    }

    /** Chat completions carry text; do not let org.json coerce malformed values into prose. */
}
