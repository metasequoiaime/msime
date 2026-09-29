import { utf8ByteLength } from "../core/text";
import parse from "postcss/lib/parse";
import { atRule, type AtRule, type ChildNode, type Root } from "postcss";
import { decodeCssUrl, rebaseCssResources } from "./css-image-value";

const MAX_IMPORTS = 16;
const MAX_DEPTH = 8;
const MAX_BYTES = 16 * 1024 * 1024;

export type ToolbarImportReader = (relative: string) => Promise<string>;

type ImportSpec = {
  relative: string;
  layer: string | null | undefined;
  supports: string | undefined;
  media: string | undefined;
};

function consumeTrivia(value: string): string | null {
  for (;;) {
    value = value.replace(/^[ \t\r\n\f]+/, "");
    if (!value.startsWith("/*")) return value;
    const end = value.indexOf("*/", 2);
    if (end < 0) return null;
    value = value.slice(end + 2);
  }
}

function consumeString(value: string): { raw: string; rest: string; quoted: boolean } | null {
  const quote = value[0];
  if (quote !== '"' && quote !== "'") return null;
  for (let index = 1; index < value.length; index++) {
    if (value[index] === "\\") index++;
    else if (value[index] === quote)
      return { raw: value.slice(1, index), rest: value.slice(index + 1), quoted: true };
  }
  return null;
}

function consumeFunction(value: string, name: string): { raw: string; rest: string } | null {
  const head = new RegExp(`^${name}\\s*\\(`, "i").exec(value);
  if (!head) return null;
  let quote = "",
    depth = 1;
  for (let index = head[0].length; index < value.length; index++) {
    const char = value[index];
    if (char === "\\") {
      index++;
      continue;
    }
    if (quote) {
      if (char === quote) quote = "";
      continue;
    }
    if (char === '"' || char === "'") quote = char;
    else if (char === "(") depth++;
    else if (char === ")" && --depth === 0)
      return {
        raw: value.slice(head[0].length, index).trim(),
        rest: value.slice(index + 1),
      };
  }
  return null;
}

function packagePath(base: string, relative: string): string | null {
  if (!relative || relative.length > 256 || /^(?:[a-z][a-z0-9+.-]*:|\/|\\)/i.test(relative))
    return null;
  const parts = base.split("/").filter(Boolean);
  parts.pop();
  for (const part of relative.replace(/^\.\//, "").split("/")) {
    if (!part || part === "." || !/^[a-zA-Z0-9._-]+$/.test(part)) return null;
    if (part === "..") {
      if (!parts.length) return null;
      parts.pop();
    } else parts.push(part);
  }
  const result = parts.join("/");
  return result.length <= 256 ? result : null;
}

function importSpec(params: string, base: string): ImportSpec | null {
  const leading = consumeTrivia(params);
  if (leading === null) return null;
  params = leading;
  const quoted = consumeString(params);
  const url = quoted ? null : consumeFunction(params, "url");
  const source = quoted?.raw ?? url?.raw;
  if (source === undefined) return null;
  const decoded = decodeCssUrl(source, Boolean(quoted) || /^['"]/.test(source));
  const relative = decoded && packagePath(base, decoded.replace(/^['"]|['"]$/g, ""));
  if (!relative) return null;
  let rest = consumeTrivia(quoted?.rest ?? url?.rest ?? "");
  if (rest === null) return null;
  let layer: string | null | undefined;
  if (/^layer(?:\s|\/\*|\(|$)/i.test(rest)) {
    const named = consumeFunction(rest, "layer");
    if (named) {
      if (!named.raw) return null;
      layer = named.raw;
      rest = consumeTrivia(named.rest);
    } else {
      layer = null;
      rest = consumeTrivia(rest.slice(5));
    }
    if (rest === null) return null;
  }
  let supports: string | undefined;
  if (/^supports\s*\(/i.test(rest)) {
    const condition = consumeFunction(rest, "supports");
    if (!condition || !condition.raw) return null;
    supports = condition.raw;
    rest = consumeTrivia(condition.rest);
    if (rest === null) return null;
  }
  return { relative, layer, supports, media: rest || undefined };
}

function wrapped(nodes: ChildNode[], spec: ImportSpec): ChildNode[] {
  let result = nodes;
  for (const condition of [
    spec.media ? { name: "media", params: spec.media } : null,
    spec.supports ? { name: "supports", params: `(${spec.supports})` } : null,
    spec.layer !== undefined ? { name: "layer", params: spec.layer ?? "" } : null,
  ]) {
    if (!condition) continue;
    const wrapper = atRule(condition);
    wrapper.append(result);
    result = [wrapper];
  }
  return result;
}

export async function prepareToolbarImports(
  css: string,
  entry: string,
  read?: ToolbarImportReader,
): Promise<{ css: string; partial: boolean }> {
  if (!entry || entry.length > 256 || utf8ByteLength(css) > MAX_BYTES)
    return { css: "", partial: true };
  let imports = 0,
    bytes = utf8ByteLength(css),
    partial = false;
  const active = new Set([entry]);

  async function expand(source: string, filename: string, depth: number): Promise<Root> {
    let root: Root;
    try {
      root = parse(source, { from: undefined, map: false }) as Root;
    } catch {
      partial = true;
      return parse("", { from: undefined, map: false }) as Root;
    }
    root.walkAtRules((rule) => {
      if (rule.name.toLowerCase() === "charset") rule.remove();
    });
    for (const declaration of root.nodes.flatMap(function collect(node): ChildNode[] {
      return "nodes" in node && Array.isArray(node.nodes)
        ? [node, ...node.nodes.flatMap(collect)]
        : [node];
    })) {
      if (declaration.type !== "decl") continue;
      const rebased = rebaseCssResources(declaration.value, filename);
      if (rebased === null) partial = true;
      else declaration.value = rebased;
    }
    let importsAllowed = true;
    for (const node of [...root.nodes]) {
      if (node.type === "comment") continue;
      const rule = node.type === "atrule" ? (node as AtRule) : null;
      if (rule?.name.toLowerCase() !== "import") {
        // CSS permits only @charset and statement-form @layer before imports.
        // @charset has already been removed; every other rule closes the import phase.
        if (!(rule?.name.toLowerCase() === "layer" && !rule.nodes)) importsAllowed = false;
        continue;
      }
      // A browser ignores imports after a qualified rule or another disallowed
      // at-rule. Do not make such an import effective merely by flattening it.
      if (!importsAllowed) {
        rule.remove();
        continue;
      }
      const spec = importSpec(rule.params, filename);
      if (
        !read ||
        !spec ||
        depth >= MAX_DEPTH ||
        imports >= MAX_IMPORTS ||
        active.has(spec?.relative ?? "")
      ) {
        partial = true;
        rule.remove();
        continue;
      }
      try {
        imports++;
        active.add(spec.relative);
        const imported = await read(spec.relative);
        bytes += utf8ByteLength(imported);
        if (bytes > MAX_BYTES) throw new Error("stylesheet budget exceeded");
        const child = await expand(imported, spec.relative, depth + 1);
        active.delete(spec.relative);
        rule.replaceWith(
          ...wrapped(
            child.nodes.map((node) => node.clone()),
            spec,
          ),
        );
      } catch {
        active.delete(spec.relative);
        partial = true;
        rule.remove();
      }
    }
    return root;
  }

  const root = await expand(css, entry, 0);
  return { css: root.toString(), partial };
}
