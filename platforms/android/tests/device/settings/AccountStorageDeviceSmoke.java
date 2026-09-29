package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.content.SharedPreferences;
import android.os.Bundle;
import app.msime.android.AndroidAccountSessionStorage;

/** Verifies encrypted session persistence inside the isolated fixture package. */
public final class AccountStorageDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        AndroidAccountSessionStorage storage =
            new AndroidAccountSessionStorage(getContext());
        try {
            storage.clear();
            String tokenMarker = "fixture-token-marker";
            String payload = "{\"version\":1,\"access\":\"" + tokenMarker
                + "\",\"refresh\":\"fixture-refresh-marker\"}";
            storage.save(payload);
            if (!payload.equals(storage.load())) throw new AssertionError("roundtrip");
            SharedPreferences preferences = getContext().getSharedPreferences(
                AndroidAccountSessionStorage.PREFERENCES_NAME, Context.MODE_PRIVATE);
            for (Object value : preferences.getAll().values()) {
                if (String.valueOf(value).contains(tokenMarker))
                    throw new AssertionError("plaintext");
            }
            storage.clear();
            if (storage.load() != null) throw new AssertionError("clear");
            boolean rejected = false;
            try {
                storage.save("x".repeat(AndroidAccountSessionStorage.MAX_PAYLOAD_BYTES + 1));
            } catch (IllegalArgumentException expected) {
                rejected = true;
            }
            if (!rejected) throw new AssertionError("payload bound");
            result.putString("stream",
                "MSIME_DEVICE_SMOKE_PASSED: encrypted account storage roundtrip, plaintext exclusion, clear and bound\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: account secure storage ("
                + error.getClass().getSimpleName() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        } finally {
            try {
                storage.clear();
            } catch (Exception ignored) {
                // The fixed failure line above remains free of storage details.
            }
        }
    }
}
