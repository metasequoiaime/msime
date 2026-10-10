/**
 * Reading what a speech provider sent back.
 *
 * The recognizers own the microphone, the socket and the session generation; none of that can run
 * without credentials and real audio. What a reply *means* needs neither, and it is the part most
 * likely to be wrong: Doubao can report an error in two places and carry its text in two more, and
 * a final frame with no text still has to end the recording rather than leave it listening.
 *
 * Separated so that half can be tested. A provider round trip stays unverified here; deciding that
 * `payload_msg.code` of 1002 is a failure, or that an empty last frame is still the last frame,
 * does not have to be.
 */
import { VoiceRecognitionPolicy } from "./VoiceRecognitionPolicy";
import { utf8Length } from "../Utf8";

const MAX_RESPONSE_BYTES: number = 1024 * 1024;

/** What the host should do about one reply. */
export interface VoiceOutcome {
  /** Recognized text, already bounded and normalised; empty when there is none. */
  readonly text: string;
  /** Set when the reply is a refusal; the host shows this and stops. */
  readonly failure: string;
  /** Whether this reply ends the recording. */
  readonly last: boolean;
}

export class VoiceResponsePolicy {
  /**
   * A batch transcription reply.
   *
   * The size gate is on the raw body rather than the decoded text: a provider that answers with a
   * megabyte of JSON is not one to hand to a parser first and judge afterwards.
   */
  static batchResult(responseCode: number, body: string | null): VoiceOutcome {
    if (
      responseCode < 200 ||
      responseCode >= 300 ||
      body === null ||
      utf8Length(body) > MAX_RESPONSE_BYTES
    ) {
      return { text: "", failure: "语音服务未返回有效识别结果。", last: true };
    }
    let text: string = "";
    try {
      const document: Record<string, Object> = JSON.parse(body) as Record<string, Object>;
      const value: Object | undefined = document.text ?? VoiceResponsePolicy.chatContent(document);
      text = VoiceRecognitionPolicy.result(typeof value === "string" ? (value as string) : "");
    } catch (error) {
      return { text: "", failure: "语音服务未返回有效识别结果。", last: true };
    }
    return text.length === 0
      ? { text: "", failure: "语音服务未识别到文字。", last: true }
      : { text: text, failure: "", last: true };
  }

  /** chat_audio 回答里的文字：`choices[0].message.content`；没有时为 undefined。 */
  private static chatContent(document: Record<string, Object>): Object | undefined {
    const choices: Object | undefined = document.choices;
    if (!Array.isArray(choices) || choices.length === 0) return undefined;
    const choice: Record<string, Object> | null = VoiceResponsePolicy.record(choices[0]);
    const message: Record<string, Object> | null =
      choice === null ? null : VoiceResponsePolicy.record(choice.message);
    return message === null ? undefined : message.content;
  }

  /**
   * One decoded Doubao frame.
   *
   * `last` comes from the frame header rather than the payload, so it is passed in: a final frame
   * carrying no text still ends the recording, and treating it as "nothing happened" would leave
   * the microphone open.
   */
  static streamingFrame(payload: string, last: boolean): VoiceOutcome {
    let document: Record<string, Object> | null;
    try {
      document = VoiceResponsePolicy.record(JSON.parse(payload));
    } catch (error) {
      return { text: "", failure: "豆包返回了无效 JSON。", last: last };
    }
    if (document === null) {
      return { text: "", failure: "豆包返回了无效响应。", last: last };
    }
    if (VoiceResponsePolicy.refused(document)) {
      return { text: "", failure: "豆包语音识别返回错误。", last: last };
    }
    const nested: Record<string, Object> | null = VoiceResponsePolicy.record(document.payload_msg);
    if (nested !== null && VoiceResponsePolicy.refused(nested)) {
      return { text: "", failure: "豆包语音识别返回错误。", last: last };
    }
    // The text sits beside the code at whichever level answered; a provider may use either.
    const result: Record<string, Object> | null =
      VoiceResponsePolicy.record(document.result) ??
      (nested === null ? null : VoiceResponsePolicy.record(nested.result));
    const raw: Object | undefined = result === null ? undefined : result.text;
    const text: string = VoiceRecognitionPolicy.result(
      typeof raw === "string" ? (raw as string) : "",
    );
    return { text: text, failure: "", last: last };
  }

  /** A non-zero code or any error member is a refusal; absent members are not. */
  private static refused(document: Record<string, Object>): boolean {
    const code: Object | undefined = document.code;
    if (typeof code === "number" && (code as number) !== 0) {
      return true;
    }
    const error: Object | undefined = document.error;
    return error !== undefined && error !== null;
  }

  private static record(value: unknown): Record<string, Object> | null {
    return value !== undefined &&
      value !== null &&
      typeof value === "object" &&
      !Array.isArray(value)
      ? (value as Record<string, Object>)
      : null;
  }
}
