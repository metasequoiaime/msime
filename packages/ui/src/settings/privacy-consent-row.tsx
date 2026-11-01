import { ActionButton } from "./action-button";
import { Row } from "../core/platform-controls";
import type { PrivacyConsentClient } from "../core/host-contracts";
import * as doc from "./document-style";

/** 宿主在没有隐私政策授权时拒绝联网请求所用的错误码（HarmonyOS 的 `PrivacyConsentPolicy.refusal`）。 */
export const PRIVACY_CONSENT_REQUIRED = "privacy_consent_required";

/** 联网请求因为没有授权被拒时给用户看的话。 */
export const PRIVACY_CONSENT_REQUIRED_MESSAGE =
  "需要先同意隐私政策才能联网。可在「我的 › 隐私」中查看并同意。";

/**
 * 「隐私政策授权」这一行：已同意时说明联网功能已开启，未同意时说明哪些联网功能关着、本机打字不受影响，按钮重新打开宿主的授权弹窗。宿主没有提供授权（`client.privacyConsent` 缺省）时不画。
 */
export function PrivacyConsentRow({ consent }: { consent: PrivacyConsentClient | undefined }) {
  if (!consent) return null;
  const granted = consent.granted;
  return (
    <Row
      title="隐私政策授权"
      detail={
        granted
          ? "已同意，联网功能已开启。"
          : "未同意：登录、云同步、社区、检查更新等联网功能已关闭，本机打字不受影响。"
      }
    >
      <ActionButton
        action={() => consent.review().catch(() => undefined)}
        className={doc.rowButton}
        ariaLabel={granted ? "查看或撤回隐私政策授权" : "查看并同意隐私政策"}
        label={granted ? "查看" : "查看并同意"}
      />
    </Row>
  );
}
