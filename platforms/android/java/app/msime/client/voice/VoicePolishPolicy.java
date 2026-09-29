package app.msime.client;


/**
 * Whether a transcript should be polished, and the exact request that does it.
 *
 * <p>Polishing is the optional second step the source offers after transcription: the recognised
 * text goes to the user's own AI service with one of the shipped prompts — 精炼整理, 忠实校对,
 * 中翻英, 口语整理 — or a prompt they wrote. It applies to whatever produced the transcript, so a
 * result from the platform recognizer is polished too when the user asked for it.
 *
 * <p>The prompt bodies are not here. They carry their own prompt-injection wording and already
 * exist in four places in this repository; this host reads the shared C++ table through
 * {@code NativeClient.polishPrompt} rather than adding a fifth copy that can drift.
 */
public final class VoicePolishPolicy {
    /** The tag pair the shipped prompts refer to, and the boundary they tell the model to respect. */
    private static final String OPEN = "<asr_text>\n";
    private static final String CLOSE = "\n</asr_text>";
    private static final int MAX_TEXT_BYTES = 32 * 1024;
    private static final int MAX_PROMPT_BYTES = 8192;

    private VoicePolishPolicy() {}

    /**
     * Whether the user asked for polishing at all.
     *
     * <p>Two switches, because the settings page has carried both: `polish_enabled` is the
     * provider-level one and `polish_text` the per-result one, and the other hosts treat either
     * being on as "polish this".
     */
    public static boolean requested(boolean polishEnabled, boolean polishText) {
        return polishEnabled || polishText;
    }

    /** Whether this host can run the request as configured. Same scheme rule as transcription. */
    public static boolean usable(String endpoint, String model, String token, String prompt) {
        return endpoint != null && endpoint.startsWith("https://") && endpoint.length() <= 2048
            && !TextPolicy.hasControl(endpoint)
            && model != null && !model.trim().isEmpty() && model.length() <= 512
            && !TextPolicy.hasControl(model)
            && token != null && !token.trim().isEmpty() && token.length() <= 16 * 1024
            && !TextPolicy.hasControl(token)
            && prompt != null && !prompt.trim().isEmpty()
            && TextPolicy.utf8Length(prompt) <= MAX_PROMPT_BYTES;
    }

    /** Whether a transcript is worth sending: empty or absurdly long is not. */
    public static boolean sendable(String text) {
        return text != null && !text.trim().isEmpty()
            && TextPolicy.utf8Length(text) <= MAX_TEXT_BYTES;
    }

    /**
     * The transcript as the user message, inside the tags the prompts name.
     *
     * <p>The wrapper is the injection boundary, not decoration: every shipped prompt tells the
     * model that what is inside these tags is data rather than instructions, and without them a
     * transcript that happens to read like an instruction has nothing marking it as not one.
     */
    public static String userMessage(String text) {
        return OPEN + (text == null ? "" : text) + CLOSE;
    }

    /**
     * The OpenAI-compatible chat request body.
     *
     * <p>`temperature` is low and `stream` is off deliberately: this is a rewrite of text the user
     * already has, so an inventive answer is a worse answer, and there is nothing to stream to —
     * the result is handed over in one piece.
     */
    public static String requestBody(String model, String prompt, String text) {
        return "{\"model\":" + json(model)
            + ",\"stream\":false,\"temperature\":0.2,\"messages\":["
            + "{\"role\":\"system\",\"content\":" + json(prompt) + "},"
            + "{\"role\":\"user\",\"content\":" + json(userMessage(text)) + "}]}";
    }

    /** Minimal JSON string escaping, so a prompt cannot break out of the document it travels in. */
    public static String json(String value) {
        String source = value == null ? "" : value;
        StringBuilder out = new StringBuilder(source.length() + 2).append('"');
        for (int index = 0; index < source.length(); index++) {
            char character = source.charAt(index);
            switch (character) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                default -> {
                    if (character < 0x20) out.append(String.format("\\u%04x", (int) character));
                    else out.append(character);
                }
            }
        }
        return out.append('"').toString();
    }

}
