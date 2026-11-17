import client from 'libmsimeclient.so';
import { OnlineQuery } from './OnlineCandidatePolicy';

interface CandidateParseRequest {
  op: string;
  query: OnlineQuery;
  body: string;
  limit: number;
}

interface CandidateParseReply {
  ok: boolean;
  value: string[] | null;
}

export class AiCandidateResponse {
  static parse(body: string, queryDocument: string, limit: number): string[] | null {
    const request: CandidateParseRequest = {
      op: 'parse', query: JSON.parse(queryDocument) as OnlineQuery, body: body, limit: limit
    };
    const reply: CandidateParseReply = JSON.parse(client.aiCandidateProtocol(
      JSON.stringify(request))) as CandidateParseReply;
    return reply.ok === true ? reply.value : null;
  }
}
