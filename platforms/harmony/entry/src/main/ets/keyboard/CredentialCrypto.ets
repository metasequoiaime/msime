import { KeyboardLog } from './KeyboardLog';
import cryptoFramework from '@ohos.security.cryptoFramework';
import util from '@ohos.util';

/**
 * Small, promise-based wrappers for signing provider probe requests. Secrets stay in the
 * native settings bridge and are passed only to CryptoFramework; this module never logs or
 * serializes a key.
 */
export async function hmacSha256Hex(key: string, message: string): Promise<string> {
  const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
  return hex(await hmacSha256Bytes(encoder.encodeInto(key), encoder.encodeInto(message)));
}

export async function hmacSha256Bytes(key: Uint8Array, message: Uint8Array): Promise<Uint8Array> {
  try {
    const generator: cryptoFramework.SymKeyGenerator = cryptoFramework.createSymKeyGenerator('HMAC');
    const keyObject: cryptoFramework.SymKey = await generator.convertKey({ data: key });
    const mac: cryptoFramework.Mac = cryptoFramework.createMac('HMAC|SHA256');
    await mac.init(keyObject);
    await mac.update({ data: message });
    const output: cryptoFramework.DataBlob = await mac.doFinal();
    return output.data;
  } catch (error) {
    KeyboardLog.error('keyboard.CredentialCrypto.hmacSha256Bytes failed');
    throw error as Error;
  }
}

export async function md5Hex(value: string): Promise<string> {
  try {
    const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
    const digest: cryptoFramework.Md = cryptoFramework.createMd('MD5');
    await digest.update({ data: encoder.encodeInto(value) });
    const output: cryptoFramework.DataBlob = await digest.digest();
    return hex(output.data);
  } catch (error) {
    KeyboardLog.error('keyboard.CredentialCrypto.md5Hex failed');
    throw error as Error;
  }
}

export async function sha256Hex(value: string): Promise<string> {
  try {
    const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
    const digest: cryptoFramework.Md = cryptoFramework.createMd('SHA256');
    await digest.update({ data: encoder.encodeInto(value) });
    const output: cryptoFramework.DataBlob = await digest.digest();
    return hex(output.data);
  } catch (error) {
    KeyboardLog.error('keyboard.CredentialCrypto.sha256Hex failed');
    throw error as Error;
  }
}

export async function tencentAuthorization(secretId: string, secretKey: string,
  timestamp: number, date: string, payload: string): Promise<string> {
  const payloadHash: string = await sha256Hex(payload);
  const canonical: string = `POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:tmt.tencentcloudapi.com\n` +
    `x-tc-action:texttranslatebatch\n\ncontent-type;host;x-tc-action\n${payloadHash}`;
  const scope: string = `${date}/tmt/tc3_request`;
  const stringToSign: string = `TC3-HMAC-SHA256\n${timestamp}\n${scope}\n${await sha256Hex(canonical)}`;
  const encoder: util.TextEncoder = util.TextEncoder.create('utf-8');
  const dateKey: Uint8Array = await hmacSha256Bytes(encoder.encodeInto(`TC3${secretKey}`), encoder.encodeInto(date));
  const serviceKey: Uint8Array = await hmacSha256Bytes(dateKey, encoder.encodeInto('tmt'));
  const signingKey: Uint8Array = await hmacSha256Bytes(serviceKey, encoder.encodeInto('tc3_request'));
  const signature: string = hex(await hmacSha256Bytes(signingKey, encoder.encodeInto(stringToSign)));
  return `TC3-HMAC-SHA256 Credential=${secretId}/${scope}, ` +
    `SignedHeaders=content-type;host;x-tc-action, Signature=${signature}`;
}

export async function niutransAuth(appId: string, apiKey: string, timestamp: string,
  sourceText: string): Promise<string> {
  return md5Hex(`apikey=${apiKey}&appId=${appId}&from=zh&srcText=${sourceText}` +
    `&timestamp=${timestamp}&to=en`);
}

function hex(bytes: Uint8Array): string {
  let result: string = '';
  for (const byte of bytes) {
    result += byte.toString(16).padStart(2, '0');
  }
  return result;
}
