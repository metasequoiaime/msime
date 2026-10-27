import { authentication } from '@kit.AccountKit';
import { BusinessError } from '@kit.BasicServicesKit';
import common from '@ohos.app.ability.common';
import util from '@ohos.util';
import { KeyboardLog, describeError } from '../keyboard/KeyboardLog';
import {
  HUAWEI_ID_PERMISSIONS, HUAWEI_ID_SCOPES, HuaweiChallenge, huaweiChallenge, huaweiChallengeAction,
  huaweiIdConfigurationError, huaweiIdErrorCode, huaweiLoginAction, responseIdToken, stateMatches
} from './HuaweiIdSignInPolicy';

interface AccountReply {
  ok?: boolean;
  error?: string;
}

/**
 * 鸿蒙设置应用里的华为账号登录（Account Kit）：经 Rust 申请带 nonce 的 challenge，用 `AuthorizationWithHuaweiIDRequest` 拉起系统的华为账号授权页拿 ID Token，再经 Rust 交给服务端的 `/v1/auth/login`。服务端按 ID Token 的 `sub`（UnionID）认人，验签、受众和 nonce 都在服务端。
 *
 * 只要 `openid` 和 `idtoken`：不申请头像昵称、手机号这类要在 AppGallery Connect 另外申请权限的 scope，也不要授权码，服务端不换码、不需要 Client Secret。应用的 Client ID 由 `module.json5` 的 `client_id` 元数据交给 Account Kit（与 APP ID 相同时可以不配），服务端 `auth.huawei.client_ids` 里要有同一个值，否则 ID Token 的受众对不上。
 *
 * 哪些判定不在这里写：challenge 的回答、nonce 的形状、响应核对和错误码换算都在 `HuaweiIdSignInPolicy`，那里可以在 node 下测试；这里只做 Account Kit 调用。授权页由系统管理，用户关掉它时 Account Kit 以取消结束，所以没有单独的取消入口。
 */
export class HarmonyHuaweiSignIn {
  private readonly context: common.UIAbilityContext;
  private readonly account: (action: string) => Promise<string>;

  constructor(context: common.UIAbilityContext, account: (action: string) => Promise<string>) {
    this.context = context;
    this.account = account;
  }

  /** 完成一次华为账号登录，返回与 Rust `login` 同形状的回复文档（`{user}`）。 */
  async signIn(): Promise<string> {
    KeyboardLog.info('huawei sign-in: started');
    const challengeDocument: string = await this.account(huaweiChallengeAction());
    const challenge: HuaweiChallenge | null = huaweiChallenge(challengeDocument);
    if (challenge === null) {
      const refused: AccountReply = JSON.parse(challengeDocument) as AccountReply;
      KeyboardLog.warn(`huawei sign-in: challenge refused ${refused.error ?? '-'}`);
      return refused.ok === true ? this.failure('account_unavailable') : challengeDocument;
    }
    const request: authentication.AuthorizationWithHuaweiIDRequest =
      new authentication.HuaweiIDProvider().createAuthorizationWithHuaweiIDRequest();
    request.scopes = HUAWEI_ID_SCOPES.slice();
    request.permissions = HUAWEI_ID_PERMISSIONS.slice();
    // 未登录华为账号时先拉起系统登录页，已登录但没授权过本应用时拉起授权页。
    request.forceAuthorization = true;
    request.state = util.generateRandomUUID(false);
    // 服务端按这个 nonce 认 challenge：不带时 ID Token 里是缺省的「default」，登录一定失败。
    request.nonce = challenge.nonce;
    // PS256 是 Account Kit 的缺省值和推荐值，服务端 PS256、RS256 都接受；写明是为了不依赖缺省值。
    request.idTokenSignAlgorithm = authentication.IdTokenSignAlgorithm.PS256;
    let idToken: string | null = null;
    try {
      const controller: authentication.AuthenticationController =
        new authentication.AuthenticationController(this.context);
      const response: authentication.AuthorizationWithHuaweiIDResponse =
        await controller.executeRequest(request) as authentication.AuthorizationWithHuaweiIDResponse;
      if (!stateMatches(request.state, response.state)) {
        KeyboardLog.warn('huawei sign-in: the response state does not match the request');
        return this.failure('account_unavailable');
      }
      idToken = responseIdToken(response.data?.idToken);
    } catch (error) {
      const code: number | undefined = (error as BusinessError).code;
      if (huaweiIdConfigurationError(code)) {
        // 指纹证书或权限不对是 AppGallery Connect 的配置问题，重试没有用：查应用的 Client ID（module.json5 的 client_id）和签名证书。
        KeyboardLog.error(`huawei sign-in: Account Kit refused the app configuration (${code})`);
      } else {
        KeyboardLog.warn(`huawei sign-in: authorization ended ${describeError(error)}`);
      }
      return this.failure(huaweiIdErrorCode(code));
    }
    if (idToken === null) {
      KeyboardLog.warn('huawei sign-in: the authorization carried no ID token');
      return this.failure('account_unavailable');
    }
    const login: string = await this.account(huaweiLoginAction(challenge.challengeId, idToken));
    const reply: AccountReply = JSON.parse(login) as AccountReply;
    if (reply.ok === true) {
      KeyboardLog.info('huawei sign-in: signed in');
    } else {
      KeyboardLog.warn(`huawei sign-in: the ID token reached the backend and was refused (${reply.error ?? '-'})`);
    }
    return login;
  }

  private failure(code: string): string {
    const reply: AccountReply = { ok: false, error: code };
    return JSON.stringify(reply);
  }
}
