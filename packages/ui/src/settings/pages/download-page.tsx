import { Row } from "../../core/platform-controls";
import { useSettingsForm } from "../settings-form-context";

/**
 * 原「其他平台下载」页的两行，现在放在关于页的版本组里；旧的 `download` 路由经 `settingsPageAliases` 打开关于页。设计稿按平台列出七个下载入口，这个客户端只知道一个下载页和发布记录，所以只链到这两处，不编造各平台的地址。触屏宿主在「我的」里有同样的链接，不显示这两行。
 */
export function OtherPlatformDownloadRows() {
  const { mobilePlatform, openExternalUrl, desktopDownloadUrl, platformReleasesPageUrl } =
    useSettingsForm();
  if (mobilePlatform) return null;
  return (
    <>
      <Row title="在其他设备上安装" description="在水杉输入法官网的下载页获取其他平台的版本。">
        <button
          type="button"
          className="secondary"
          onClick={() => void openExternalUrl(desktopDownloadUrl)}
        >
          打开下载页
        </button>
      </Row>
      <Row title="历史版本" description="各版本的更新说明与安装包。">
        <button
          type="button"
          className="secondary"
          onClick={() => void openExternalUrl(platformReleasesPageUrl)}
        >
          查看发布记录
        </button>
      </Row>
    </>
  );
}
