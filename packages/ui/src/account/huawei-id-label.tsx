import huaweiLogoRed from "../assets/huawei-id/logo-red.svg";
import huaweiLogoWhite from "../assets/huawei-id/logo-white.svg";
import * as account from "./account-style";

/**
 * 「华为账号登录」按钮的内容：华为标志加按钮文字，按华为账号登录按钮规范，标志在文字左边。两份标志取自 HarmonyOS SDK 的 HMS 资源（`huawei_id_logo_red.svg`、`huawei_id_logo_white.svg`），也就是系统 `LoginWithHuaweiIDButton` 自己画的那两张；ArkWeb 里用不了那个组件，所以按钮由页面画、标志用同一份。浅色主题用红色标志，深色主题下红色版的黑色「HUAWEI」字样看不清，换白色版。
 */
export function HuaweiIdLabel() {
  return (
    <span className={account.choiceLabel}>
      <img
        src={huaweiLogoWhite}
        alt=""
        aria-hidden="true"
        className="size-6 shrink-0 light-theme:hidden"
      />
      <img
        src={huaweiLogoRed}
        alt=""
        aria-hidden="true"
        className="hidden size-6 shrink-0 light-theme:block"
      />
      华为账号登录
    </span>
  );
}
