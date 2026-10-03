package app.msime.android;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Bounded reader for reply templates explicitly shared into the app-private library. */
public final class CommunityReplyLibrary {
    public static final int MAXIMUM_BYTES = 4_000_000;
    public static final int MAXIMUM_ITEMS = 50;
    private static final int MAXIMUM_CONTAINER_ITEMS = 64;
    private static final int MAXIMUM_ID_BYTES = 64 * 1024;
    private static final int MAXIMUM_NAME_CHARACTERS = 32;
    private static final int MAXIMUM_PROMPT_CHARACTERS = 2_000;
    public record Template(String id, String name, String prompt) {}

    private final Path file;

    public CommunityReplyLibrary(Path privateFilesDirectory) {
        file = privateFilesDirectory.toAbsolutePath().normalize().resolve("CommunityLibrary.json");
    }

    public List<Template> read() throws IOException {
        return read(file);
    }

    public static List<Template> read(Path file) throws IOException {
        rejectSymlinkComponents(file);
        if (!Files.exists(file, LinkOption.NOFOLLOW_LINKS)) return List.of();
        if (!Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS)) throw new IOException("Invalid community library");
        byte[] bytes = readBounded(file);
        final String json;
        try {
            json = StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString();
        } catch (CharacterCodingException error) {
            throw new IOException("Community library is not UTF-8", error);
        }
        Object decoded;
        try { decoded = new Parser(json).parse(); }
        catch (IllegalArgumentException error) { throw new IOException("Invalid community library", error); }
        if (!(decoded instanceof List<?> items) || items.size() > MAXIMUM_ITEMS)
            throw new IOException("Invalid community library");
        List<Template> replies = new ArrayList<>();
        Set<String> ids = new HashSet<>();
        for (Object value : items) {
            if (!(value instanceof Map<?, ?> item)) throw new IOException("Invalid community library");
            String id = string(item.get("id"));
            String kind = string(item.get("kind"));
            Object contentValue = item.get("content");
            if (id == null || kind == null || !(contentValue instanceof Map<?, ?> content))
                throw new IOException("Invalid community library");
            if (!"reply".equals(kind)) continue;
            String name = string(item.get("name"));
            String prompt = string(content.get("prompt"));
            if (name == null || id.isEmpty() || name.isEmpty() || prompt == null
                    || TextPolicy.blank(id) || TextPolicy.blank(name) || TextPolicy.blank(prompt)
                    || !name.equals(name.trim()) || !ids.add(id))
                throw new IOException("Invalid community library");
            if (!TextPolicy.validUnicode(id) || !TextPolicy.validUnicode(name)
                    || !TextPolicy.validUnicode(prompt) || TextPolicy.hasControl(id)
                    || TextPolicy.hasControl(name) || TextPolicy.hasControl(prompt)
                    || TextPolicy.utf8Length(id) > MAXIMUM_ID_BYTES
                    || name.codePointCount(0, name.length()) > MAXIMUM_NAME_CHARACTERS
                    || prompt.codePointCount(0, prompt.length()) > MAXIMUM_PROMPT_CHARACTERS)
                throw new IOException("Invalid community library");
            replies.add(new Template(id, name, prompt));
        }
        return List.copyOf(replies);
    }

    /** Read only the library envelope, even if a replaced file grows after inspection. */
    private static byte[] readBounded(Path file) throws IOException {
        try (InputStream input = Files.newInputStream(file)) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream(MAXIMUM_BYTES);
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() + count > MAXIMUM_BYTES)
                    throw new IOException("Community library is too large");
                bytes.write(buffer, 0, count);
            }
            return bytes.toByteArray();
        }
    }

    private static void rejectSymlinkComponents(Path path) throws IOException {
        if (path == null) throw new IOException("Community library path unavailable");
        Path absolute = path.toAbsolutePath().normalize();
        Path current = absolute.getRoot();
        if (current == null) throw new IOException("Community library path unavailable");
        for (Path component : absolute) {
            current = current.resolve(component);
            if (Files.isSymbolicLink(current) && !isTrustedMacSystemAlias(current))
                throw new IOException("Community library path contains a symbolic link");
        }
    }

    /** The host-side smoke tests run on macOS, where these two root aliases are stable. */
    private static boolean isTrustedMacSystemAlias(Path path) {
        if (!path.equals(Path.of("/tmp")) && !path.equals(Path.of("/var"))) return false;
        try {
            Path target = path.getParent().resolve(Files.readSymbolicLink(path)).normalize();
            return (path.equals(Path.of("/tmp")) && target.equals(Path.of("/private/tmp")))
                || (path.equals(Path.of("/var")) && target.equals(Path.of("/private/var")));
        } catch (IOException | SecurityException error) {
            return false;
        }
    }

    private static String string(Object value) { return value instanceof String text ? text : null; }

    private static final class Parser {
        private static final int MAXIMUM_DEPTH = 24;
        private final String input;
        private int index;

        Parser(String input) { this.input = input; }

        Object parse() {
            Object value = value(0);
            whitespace();
            if (index != input.length()) throw invalid();
            return value;
        }

        private Object value(int depth) {
            if (depth > MAXIMUM_DEPTH) throw invalid();
            whitespace();
            if (index >= input.length()) throw invalid();
            return switch (input.charAt(index)) {
                case '[' -> array(depth + 1);
                case '{' -> object(depth + 1);
                case '"' -> string();
                case 't' -> literal("true", Boolean.TRUE);
                case 'f' -> literal("false", Boolean.FALSE);
                case 'n' -> literal("null", null);
                default -> number();
            };
        }

        private List<Object> array(int depth) {
            index++;
            List<Object> values = new ArrayList<>();
            whitespace();
            if (take(']')) return values;
            while (true) {
                if (values.size() >= MAXIMUM_CONTAINER_ITEMS) throw invalid();
                values.add(value(depth));
                whitespace();
                if (take(']')) return values;
                require(',');
            }
        }

        private Map<String, Object> object(int depth) {
            index++;
            Map<String, Object> values = new LinkedHashMap<>();
            whitespace();
            if (take('}')) return values;
            while (true) {
                if (values.size() >= MAXIMUM_CONTAINER_ITEMS) throw invalid();
                whitespace();
                if (index >= input.length() || input.charAt(index) != '"') throw invalid();
                String key = string();
                whitespace();
                require(':');
                if (values.containsKey(key)) throw invalid();
                values.put(key, value(depth));
                whitespace();
                if (take('}')) return values;
                require(',');
            }
        }

        private String string() {
            require('"');
            StringBuilder result = new StringBuilder();
            while (index < input.length()) {
                char value = input.charAt(index++);
                if (value == '"') return result.toString();
                if (value < 0x20) throw invalid();
                if (value != '\\') {
                    result.append(value);
                    continue;
                }
                if (index >= input.length()) throw invalid();
                char escaped = input.charAt(index++);
                switch (escaped) {
                    case '"', '\\', '/' -> result.append(escaped);
                    case 'b' -> result.append('\b');
                    case 'f' -> result.append('\f');
                    case 'n' -> result.append('\n');
                    case 'r' -> result.append('\r');
                    case 't' -> result.append('\t');
                    case 'u' -> result.append(unicode());
                    default -> throw invalid();
                }
            }
            throw invalid();
        }

        private char unicode() {
            if (index + 4 > input.length()) throw invalid();
            int value = 0;
            for (int offset = 0; offset < 4; offset++) {
                int digit = Character.digit(input.charAt(index++), 16);
                if (digit < 0) throw invalid();
                value = value * 16 + digit;
            }
            return (char)value;
        }

        private Object number() {
            int start = index;
            if (take('-') && index >= input.length()) throw invalid();
            if (take('0')) {
                if (index < input.length() && Character.isDigit(input.charAt(index))) throw invalid();
            } else {
                digits();
            }
            if (take('.')) digits();
            if (take('e') || take('E')) {
                take('+'); take('-'); digits();
            }
            if (start == index) throw invalid();
            try { return Double.valueOf(input.substring(start, index)); }
            catch (NumberFormatException error) { throw invalid(); }
        }

        private void digits() {
            int start = index;
            while (index < input.length() && Character.isDigit(input.charAt(index))) index++;
            if (start == index) throw invalid();
        }

        private Object literal(String expected, Object value) {
            if (!input.startsWith(expected, index)) throw invalid();
            index += expected.length();
            return value;
        }

        private void whitespace() {
            while (index < input.length()) {
                char value = input.charAt(index);
                if (value != ' ' && value != '\n' && value != '\r' && value != '\t') return;
                index++;
            }
        }

        private boolean take(char expected) {
            if (index < input.length() && input.charAt(index) == expected) { index++; return true; }
            return false;
        }

        private void require(char expected) { if (!take(expected)) throw invalid(); }
        private IllegalArgumentException invalid() { return new IllegalArgumentException("Invalid JSON"); }
    }
}
