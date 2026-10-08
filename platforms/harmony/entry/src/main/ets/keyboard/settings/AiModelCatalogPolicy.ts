import { AiEndpointPolicy } from "./AiEndpointPolicy";
import { TextPolicy } from "../TextPolicy";

/** One model row returned by an OpenAI-compatible or Anthropic catalog. */
export interface AiCatalogModel {
  id?: string;
  supported_endpoint_types?: string[];
  chat_completions_bridge?: boolean;
  active?: boolean;
}

/** The bounded page shape consumed by the native settings bridge. */
export interface AiCatalogPage {
  data?: AiCatalogModel[];
  has_more?: boolean;
  last_id?: string;
}

/** Provider-specific model-catalog rules shared by the request loop and logic tests. */
export class AiModelCatalogPolicy {
  static modelsUrl(endpoint: string): string | null {
    const trimmed: string = endpoint.trim();
    // https 不限主机，http 只能指向本机或局域网，见 AiEndpointPolicy。
    if (!AiEndpointPolicy.allowed(trimmed)) {
      return null;
    }
    const queryIndex: number = trimmed.indexOf("?");
    const query: string = queryIndex >= 0 ? trimmed.substring(queryIndex) : "";
    let path: string = queryIndex >= 0 ? trimmed.substring(0, queryIndex) : trimmed;
    while (path.endsWith("/")) path = path.substring(0, path.length - 1);
    for (const suffix of ["/chat/completions", "/audio/transcriptions"]) {
      if (path.endsWith(suffix)) {
        path = path.substring(0, path.length - suffix.length);
        break;
      }
    }
    return `${path}/models${query}`;
  }

  static isAnthropic(provider: string, modelsUrl: string): boolean {
    return provider === "anthropic" || modelsUrl.startsWith("https://api.anthropic.com/");
  }

  static pageUrl(modelsUrl: string, anthropic: boolean, cursor: string): string {
    if (!anthropic) return modelsUrl;
    const queryIndex: number = modelsUrl.indexOf("?");
    const path: string = queryIndex >= 0 ? modelsUrl.substring(0, queryIndex) : modelsUrl;
    const rawQuery: string = queryIndex >= 0 ? modelsUrl.substring(queryIndex + 1) : "";
    const preserved: string[] = [];
    for (const part of rawQuery.split("&")) {
      if (part.length === 0) continue;
      const equals: number = part.indexOf("=");
      const rawName: string = equals >= 0 ? part.substring(0, equals) : part;
      let name: string = rawName;
      try {
        name = decodeURIComponent(rawName.replace(/\+/g, " "));
      } catch (_) {
        // Keep malformed provider query names unchanged; endpoint validation has already
        // rejected controls, and dropping an unrelated name would change its meaning.
      }
      if (name !== "limit" && name !== "after_id") preserved.push(part);
    }
    const base: string = path + (preserved.length > 0 ? `?${preserved.join("&")}` : "");
    const separator: string = base.includes("?") ? "&" : "?";
    const after: string = cursor.length > 0 ? `&after_id=${encodeURIComponent(cursor)}` : "";
    return `${base}${separator}limit=1000${after}`;
  }

  static accepts(model: AiCatalogModel): boolean {
    const id: string = model.id ?? "";
    if (model.active === false || id.length === 0 || id.length > 256 || TextPolicy.hasControl(id)) {
      return false;
    }
    const endpoints: string[] | undefined = model.supported_endpoint_types;
    if (endpoints === undefined || endpoints.length === 0) return true;
    return endpoints.includes("openai") || model.chat_completions_bridge === true;
  }

  static append(models: string[], page: AiCatalogPage, maximum: number = 5000): boolean {
    if (!Array.isArray(page.data)) return false;
    for (const row of page.data) {
      const id: string = row.id ?? "";
      if (!this.accepts(row) || models.includes(id)) continue;
      models.push(id);
      if (models.length > maximum) return false;
    }
    return true;
  }

  static nextCursor(page: AiCatalogPage, anthropic: boolean, previous: string[]): string | null {
    if (page.has_more !== true) return "";
    const cursor: string = page.last_id ?? "";
    if (
      !anthropic ||
      cursor.length === 0 ||
      cursor.length > 256 ||
      TextPolicy.hasControl(cursor) ||
      previous.includes(cursor)
    )
      return null;
    return cursor;
  }
}
