package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.io.BufferedInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.FutureTask;
import org.json.JSONObject;

/** 在安装产品里执行合成云请求，检查 Android HTTP 栈的字节、缓存、重定向与总时限。 */
public final class CloudCandidateTransportDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            run();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: custom cloud GET/POST, headers, cache, redirect, deadline\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: cloud transport " + error.getClass().getSimpleName() + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private String fetch(JSONObject descriptor) throws Exception {
        Class<?> transport = getTargetContext().getClassLoader().loadClass("app.msime.android.OnlineCandidateTransport");
        return (String) transport.getMethod("cloud", JSONObject.class).invoke(null, descriptor);
    }

    private void run() throws Exception {
        String answer = "{\"candidates\":[{\"text\":\"合成云候选\"}]}";
        for (String method : new String[] {"GET", "POST"}) {
            try (ServerSocket server = new ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"))) {
                server.setSoTimeout(5000);
                FutureTask<String> peer = new FutureTask<>(() -> {
                    try (Socket socket = server.accept()) {
                        socket.setSoTimeout(5000);
                        InputStream input = new BufferedInputStream(socket.getInputStream());
                        String headers = headers(input);
                        int length = 0;
                        for (String line : headers.split("\r\n")) {
                            if (line.toLowerCase(java.util.Locale.ROOT).startsWith("content-length:")) {
                                length = Integer.parseInt(line.substring(line.indexOf(':') + 1).trim());
                            }
                        }
                        ByteArrayOutputStream body = new ByteArrayOutputStream();
                        for (int i = 0; i < length; ++i) { int value = input.read(); if (value < 0) throw new AssertionError(); body.write(value); }
                        byte[] response = answer.getBytes(StandardCharsets.UTF_8);
                        socket.getOutputStream().write(("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: "
                            + response.length + "\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
                        socket.getOutputStream().write(response);
                        return headers + "\n" + body.toString("UTF-8");
                    }
                });
                Thread worker = new Thread(peer, "synthetic-cloud-peer");
                worker.setDaemon(true); worker.start();
                JSONObject descriptor = descriptor(server.getLocalPort(), method, 2000);
                if (!answer.equals(fetch(descriptor))) throw new AssertionError();
                String received = peer.get(5, java.util.concurrent.TimeUnit.SECONDS);
                if (!received.startsWith(method + " /candidate?q=ni%20hao ")
                    || !received.contains("Bearer synthetic") || received.toLowerCase(java.util.Locale.ROOT).contains("cookie:")) throw new AssertionError();
                if (method.equals("POST") && !received.endsWith(descriptor.getString("body_utf8"))) throw new AssertionError();
            }
        }
        if (!answer.equals(fetch(descriptor(1, "GET", 100).put("cached_response", answer)))) throw new AssertionError();
        for (boolean redirect : new boolean[] {true, false}) {
            try (ServerSocket server = new ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"))) {
                server.setSoTimeout(5000);
                FutureTask<Void> peer = new FutureTask<>(() -> {
                    try (Socket socket = server.accept()) {
                        socket.setSoTimeout(5000);
                        headers(socket.getInputStream());
                        if (redirect) socket.getOutputStream().write("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/redirect\r\nContent-Length: 0\r\n\r\n".getBytes(StandardCharsets.US_ASCII));
                        else Thread.sleep(1500);
                    }
                    return null;
                });
                Thread worker = new Thread(peer, "synthetic-cloud-deadline");
                worker.setDaemon(true); worker.start();
                long start = android.os.SystemClock.elapsedRealtime();
                if (fetch(descriptor(server.getLocalPort(), "GET", 200)) != null) throw new AssertionError();
                if (!redirect && android.os.SystemClock.elapsedRealtime() - start >= 1400) throw new AssertionError();
                peer.get(5, java.util.concurrent.TimeUnit.SECONDS);
            }
        }
    }

    private static JSONObject descriptor(int port, String method, int timeout) throws Exception {
        return new JSONObject().put("url", "http://127.0.0.1:" + port + "/candidate?q=ni%20hao")
            .put("method", method).put("headers", new JSONObject().put("Authorization", "Bearer synthetic")
                .put("Content-Type", "application/json")).put("body_utf8", method.equals("POST") ? "{\"query\":\"合成 nihao\"}" : "")
            .put("timeout_ms", timeout).put("connect_timeout_ms", timeout).put("max_response_bytes", 262144);
    }

    private static String headers(InputStream input) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        while (bytes.size() < 8192) {
            int value = input.read();
            if (value < 0) throw new AssertionError();
            bytes.write(value);
            String text = bytes.toString("ISO-8859-1");
            if (text.endsWith("\r\n\r\n")) return text;
        }
        throw new AssertionError();
    }
}
