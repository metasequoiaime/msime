/** 字节转 Base64；按三的倍数分块编码，避免展开大数组的参数或构造整段二进制串。 */
export function bytesToBase64(bytes: Uint8Array | readonly number[]): string {
  const chunk = 3 * 8192;
  const parts: string[] = [];
  for (let offset = 0; offset < bytes.length; offset += chunk) {
    const end = Math.min(offset + chunk, bytes.length);
    let binary = "";
    for (let index = offset; index < end; index++) binary += String.fromCharCode(bytes[index]);
    parts.push(btoa(binary));
  }
  return parts.join("");
}
