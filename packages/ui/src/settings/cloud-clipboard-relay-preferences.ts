/** 自建云剪贴板中继的地址和静态令牌偏好。 */
export type CloudClipboardRelayPreferences = {
  url: string;
  token: string;
};

/** 存放云剪贴板的 WebDAV 网盘：文件夹地址、网盘账号和应用密码（`docs/cloud-clipboard-webdav.md`）。与中继二选一。 */
export type CloudClipboardWebDavPreferences = {
  url: string;
  username: string;
  password: string;
};
