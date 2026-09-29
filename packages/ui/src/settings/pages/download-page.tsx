import { GroupList, Row } from "../../core/platform-controls";
import { useSettingsForm } from "../settings-form-context";

/**
 * The 其他平台下载 page. The design lists seven platforms with a count; this client knows one download page and the release history, so it links to those rather than inventing per-platform addresses. Touch hosts reach the same link from 我的, and do not list this page.
 */
export function DownloadSettingsPage() {
  const { page, openExternalUrl, desktopDownloadUrl, platformReleasesPageUrl } = useSettingsForm();
  return (
    <div hidden={page !== "download"} role="group" aria-label="其他平台下载">
      <GroupList title="下载">
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
      </GroupList>
    </div>
  );
}
