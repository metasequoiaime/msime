import http from '@ohos.net.http';
import { installNoRedirectGuard } from '../../network/HarmonyHttpSecurity';
import { KeyboardLog, describeError } from '../KeyboardLog';
import { TextPolicy } from '../TextPolicy';
import { utf8Length } from '../Utf8';
import { VoiceResponsePolicy } from './VoiceResponsePolicy';
import { VoicePolishPromptCatalog } from './VoicePolishPromptCatalog';
import { VoiceProviderCatalog } from './VoiceProviderCatalog';
import { VoiceInputConfiguration, VoicePolishTokens } from './VoiceInputConfiguration';
import { VoicePolishRequestPolicy } from './VoicePolishRequestPolicy';

const MAX_ENDPOINT_BYTES: number = 2048;
const MAX_MODEL_BYTES: number = 512;
const MAX_TOKEN_BYTES: number = 16384;
const MAX_PROMPT_BYTES: number = 16384;
const MAX_REQUEST_TEXT_BYTES: number = 16384;

interface PolishMessage {
  role: string;
  content: string;
}

interface PolishRequestBody {
  model: string;
  stream: boolean;
  temperature: number;
  messages: PolishMessage[];
}

/** 按需润色识别结果，不把凭据或原始响应写入日志。 */
export class HarmonyVoicePolisher {
  private request: http.HttpRequest | undefined = undefined;

  cancel(): void {
    const request: http.HttpRequest | undefined = this.request;
    this.request = undefined;
    if (request !== undefined) request.destroy();
  }

  async polish(config: VoiceInputConfiguration, text: string): Promise<string | null> {
    if (!config.polish_enabled && !config.polish_text) return null;
    const provider: string = (config.polish_provider ?? '').trim();
    const defaultEndpoint: string = VoiceProviderCatalog.polishEndpoint(provider);
    const defaultModel: string = VoiceProviderCatalog.polishModel(provider);
    const endpoint: string = (config.polish_endpoint ?? '').trim() || defaultEndpoint;
    const model: string = (config.polish_model ?? '').trim() || defaultModel;
    const token: string = (config.polish_token ?? '').trim()
      || this.tokenValue(config.polish_tokens ?? {}, provider);
    const prompt: string = this.prompt(config);
    if (!this.validEndpoint(endpoint) || defaultEndpoint.length === 0
      || model.length === 0 || utf8Length(model) > MAX_MODEL_BYTES
      || TextPolicy.hasControl(model) || token.length === 0 || utf8Length(token) > MAX_TOKEN_BYTES
      || TextPolicy.hasControl(token) || !TextPolicy.validMultiline(prompt, MAX_PROMPT_BYTES, true)
      || !TextPolicy.validMultiline(text, MAX_REQUEST_TEXT_BYTES, true)) return null;

    const body: PolishRequestBody = {
      model: model,
      stream: false,
      temperature: 0.2,
      messages: [
        { role: 'system', content: prompt },
        { role: 'user', content: VoicePolishRequestPolicy.userMessage(text) }
      ]
    };
    const request: http.HttpRequest = http.createHttp();
    installNoRedirectGuard(request);
    this.request = request;
    try {
      const response: http.HttpResponse = await request.request(endpoint, {
        method: http.RequestMethod.POST,
        header: {
          Accept: 'application/json',
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`
        },
        connectTimeout: 5000,
        readTimeout: 60000,
        extraData: JSON.stringify(body)
      });
      return VoiceResponsePolicy.polishResult(response.responseCode,
        typeof response.result === 'string' ? response.result : null);
    } catch (error) {
      KeyboardLog.error(`Harmony voice polish unavailable: ${describeError(error)}`);
      return null;
    } finally {
      if (this.request === request) this.request = undefined;
      request.destroy();
    }
  }

  // 空自定义槽位与未设置方案一样，使用内置精炼整理正文。
  private prompt(config: VoiceInputConfiguration): string {
    const slot: string = config.polish_prompt_id ?? 'cleanup';
    let custom: string | null = null;
    if (slot === 'custom_1') custom = config.polish_prompt_custom_1 ?? '';
    else if (slot === 'custom_2') custom = config.polish_prompt_custom_2 ?? '';
    else if (slot === 'custom_3') custom = config.polish_prompt_custom_3 ?? '';
    if (custom !== null) return custom.trim().length > 0 ? custom.trim() : VoicePolishPromptCatalog.promptForId('cleanup');
    return VoicePolishPromptCatalog.promptForId(slot);
  }

  private tokenValue(tokens: VoicePolishTokens, provider: string): string {
    switch (provider) {
      case 'siliconflow': return (tokens.siliconflow ?? '').trim();
      case 'openai': return (tokens.openai ?? '').trim();
      case 'deepseek': return (tokens.deepseek ?? '').trim();
      case 'groq': return (tokens.groq ?? '').trim();
      default: return '';
    }
  }

  private validEndpoint(value: string): boolean {
    return TextPolicy.validAuthority(value, ['https://'], MAX_ENDPOINT_BYTES);
  }
}
