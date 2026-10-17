import client from 'libmsimeclient.so';
import { AiEndpointCheck, AiEndpointPolicy } from './AiEndpointPolicy';

/** 经 NAPI 问 Rust 的 AI 接口地址检查，规则见 `AiEndpointPolicy.ts`。纯函数，可以在主线程上调用。 */
export class AiEndpoint {
  static check(endpoint: string): AiEndpointCheck {
    return AiEndpointPolicy.parse(client.aiEndpointCheck(endpoint));
  }

  static allowed(endpoint: string): boolean {
    return AiEndpoint.check(endpoint).result === 'allowed';
  }
}
