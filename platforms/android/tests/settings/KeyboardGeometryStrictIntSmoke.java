import app.msime.android.NumberPolicy;
package app.msime.android.test;

import app.msime.android.KeyboardGeometry;
import java.math.BigDecimal;
import org.json.JSONObject;

/** Shared geometry values must not be silently truncated by JSONObject.optInt. */
public final class KeyboardGeometryStrictIntSmoke {
    public static void main(String[] args) throws Exception {
        JSONObject values = new JSONObject()
            .put("valid", 45)
            .put("fractional", 45.5)
            .put("boolean", true)
            .put("huge", 1e40)
            .put("candidate_limit", 2.5)
            .put("timeout_ms", true);
        if (NumberPolicy.strictInt(values, "valid", -1) != 45)
            throw new AssertionError("integer geometry value was rejected");
        for (String key : new String[] {"fractional", "boolean", "huge", "candidate_limit", "timeout_ms"}) {
            if (NumberPolicy.strictInt(values, key, -1) != -1)
                throw new AssertionError("malformed geometry value accepted: " + key);
        }
        if (NumberPolicy.strictInt(new BigDecimal("45.0000000000000000001"), -1) != -1)
            throw new AssertionError("precise fractional integer was rounded");
        if (NumberPolicy.strictInt(new BigDecimal("2147483647.0000000001"), -1) != -1)
            throw new AssertionError("precise fractional upper bound was rounded");
        if (NumberPolicy.strictLong(Long.MAX_VALUE - 1, -1) != Long.MAX_VALUE - 1)
            throw new AssertionError("large exact long was rounded");
        if (NumberPolicy.strictLong(9.223372036854776E18, -1) != -1)
            throw new AssertionError("rounded double was accepted as a long");
    }
}
