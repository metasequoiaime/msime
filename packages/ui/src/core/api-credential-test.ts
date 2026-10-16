export type ApiCredentialTestService =
  | "translation.tencent"
  | "translation.niutrans"
  | "translation.custom"
  | "voice.asr"
  | "voice.polish"
  | "ai.assistant";
export type ApiCredentialTestResult = { ok: boolean; message: string };
