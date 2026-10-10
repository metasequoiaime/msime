/**
 * 账号头像的下载规则，照 client-core 的 `account/avatar.rs`（桌面的 `account_avatar` 用它）写成，鸿蒙的请求走系统 HTTPS 栈，规则在这里各写一份。
 *
 * 设置页的内容安全策略只放行 `data:` 图片，所以头像由宿主下载、校验后转成 `data:` 地址交给页面。只下载服务端给出的、指向头像存储桶或 Google 头像主机的 HTTPS 地址，不跟随重定向，只接受大小上限内、文件头确实是 PNG、JPEG 或 WebP 的内容。
 */

/** 上传的头像所在的公开存储桶。 */
const UPLOADED_AVATAR_HOST: string = "media.msime.app";
/** Google 头像主机及其所有子域名（lh3.、lh4. 等）。 */
const GOOGLE_AVATAR_HOST: string = "googleusercontent.com";

export class AccountAvatarPolicy {
  /** 显示用头像的下载上限。存储的头像只有几十 KiB，这条上限只是防止宿主读到别的东西。与 `MAX_ACCOUNT_AVATAR_FETCH_BYTES` 相同。 */
  static readonly MAX_BYTES: number = 2 * 1024 * 1024;

  /** 请求头 `Accept`：只要这三种图片。 */
  static readonly ACCEPT: string = "image/png, image/jpeg, image/webp";

  /** `url` 的主机名：只认没有用户信息、没有端口的 `https://主机/...`，其余返回 null。 */
  private static host(url: string): string | null {
    const match: RegExpExecArray | null = /^https:\/\/([^/?#]*)(?:[/?#]|$)/.exec(url);
    if (match === null) return null;
    const authority: string = match[1];
    if (authority.length === 0 || authority.includes("@") || authority.includes(":")) return null;
    return authority.toLowerCase();
  }

  /** 这个地址是不是本客户端会去下载的头像：HTTPS，不带凭据和端口，主机是头像存储桶或 Google 头像主机。 */
  static allowed(url: string): boolean {
    if (typeof url !== "string" || /[\s\\]/.test(url)) return false;
    const host: string | null = AccountAvatarPolicy.host(url);
    if (host === null) return false;
    return (
      host === UPLOADED_AVATAR_HOST ||
      host === GOOGLE_AVATAR_HOST ||
      host.endsWith(`.${GOOGLE_AVATAR_HOST}`)
    );
  }

  /** 按文件头判断图片类型，不看响应头怎么说；不是 PNG、JPEG 或 WebP 时返回 null。 */
  static sniff(bytes: Uint8Array): string | null {
    const starts = (prefix: number[]): boolean =>
      bytes.length >= prefix.length &&
      prefix.every((value: number, index: number) => bytes[index] === value);
    if (starts([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) return "image/png";
    if (starts([0xff, 0xd8, 0xff])) return "image/jpeg";
    if (
      bytes.length >= 12 &&
      starts([0x52, 0x49, 0x46, 0x46]) &&
      bytes[8] === 0x57 &&
      bytes[9] === 0x45 &&
      bytes[10] === 0x42 &&
      bytes[11] === 0x50
    ) {
      return "image/webp";
    }
    return null;
  }

  /** 下载结果能不能当头像：状态 200、非空、不超过上限、文件头是允许的图片。能用时返回它的类型，否则返回 null。 */
  static accept(status: number, bytes: Uint8Array): string | null {
    if (status !== 200 || bytes.length === 0 || bytes.length > AccountAvatarPolicy.MAX_BYTES)
      return null;
    return AccountAvatarPolicy.sniff(bytes);
  }
}
