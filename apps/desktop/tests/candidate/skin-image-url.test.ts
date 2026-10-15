import { expect, test } from "vitest";
import { skinImageUrl } from "../../../../packages/ui/src/skin/skin-image";

test.each([0, 24577, 49154])(
  "skin data URLs preserve all %s bytes and their MIME type",
  (length) => {
    const bytes = Array.from({ length }, (_, index) => (index * 31 + 7) & 0xff);
    const encoded = btoa(bytes.map((byte) => String.fromCharCode(byte)).join(""));
    expect(skinImageUrl({ contentType: "image/svg+xml", bytes })).toBe(
      `data:image/svg+xml;base64,${encoded}`,
    );
  },
);
