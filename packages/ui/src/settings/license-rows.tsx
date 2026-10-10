import * as doc from "./document-style";
import { LinkRow, Row } from "../core/platform-controls";

export interface LicenseRowsProps {
  openThirdPartyLicenses?: () => Promise<void>;
}

/** 「关于 › 许可与隐私」里的版权和第三方声明行，macOS 和 Windows 显示。 */
export function LicenseRows({ openThirdPartyLicenses }: LicenseRowsProps) {
  return (
    <>
      <Row
        title="许可与版权"
        description="水杉 IME 以 GPL-3.0 发布；第三方组件许可随应用资源提供。"
      >
        <span className={doc.version} aria-label="版权">
          © 2026 Metasequoia IME
        </span>
      </Row>
      {openThirdPartyLicenses && (
        <LinkRow title="第三方组件许可" external onClick={() => void openThirdPartyLicenses()} />
      )}
    </>
  );
}
