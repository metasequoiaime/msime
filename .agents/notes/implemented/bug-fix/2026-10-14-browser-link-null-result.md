# Agent Note: 浏览器隔离新标签页的 null 返回值不代表失败

Status: implemented

## Problem

`useExternalUrl` 在没有原生打开链接接口时调用 `window.open(url, "_blank", "noopener,noreferrer")`，随后把空返回值解释为弹窗被阻止。`noopener` 本身就要求成功打开时返回 `null`，因此正常访问也显示「无法打开外部链接，请稍后重试」。现有成功测试用 `{}` 模拟返回值，掩盖了浏览器契约。

## Decision

保留 HTTPS 地址校验及 `noopener,noreferrer`，不根据浏览器返回的窗口引用判断失败。浏览器抛异常、原生打开链接接口拒绝，以及非法地址继续进入原有错误提示。成功测试改用真实的 `null` 契约，补上浏览器异常与宿主失败回归。

## Alternatives considered

移除隔离参数后查看返回引用，可以区分通常的弹窗拦截。但这让外部页面获得 `opener`，并可能发送来源信息，为了检测窗口引用而撤掉隔离不可取。

改成临时 `<a target="_blank" rel="noopener noreferrer">` 再触发点击，也能安全导航。但链接点击同样不提供可靠的打开结果，还引入 DOM 创建和清理；现有 `window.open` 已覆盖导航，无需更换机制。

## Consequences

带隔离参数的返回值不能区分成功和浏览器拦截，因此不能据此给弹窗拦截报错。这里不把发出打开请求当作已经确认页面加载成功，也不新增无法可靠判断的提示。

正常打开的新标签页不再触发失败提示，原生宿主失败和浏览器抛异常仍显示原有提示。代价是浏览器静默拦截时没有可靠的单次返回结果可以用于报告。

## Verification

`use-external-url.test.ts` 把成功测试的模拟返回值改为 `null`，旧实现失败：安全打开调用后仍触发错误提示。修复后六项全部通过，包含非法地址、浏览器异常和原生宿主拒绝；与隐私页、关于页、服务商链接入口一起运行共 19 项通过。

Playwright 使用缓存的 Chromium headless 执行完整 `use-external-url.ts` 源码（只移除 TypeScript 类型和导出标记），从合成按钮访问被拦截响应的 `https://example.test/docs`。旧实现的目的页面正常打开、`opener` 为 `null`，但错误列表含失败提示；修复后仍正常打开、`opener` 仍为 `null`，错误列表为空。没有连接真实外部服务或读取用户浏览器资料。

前端类型检查与生产构建、改动文件 lint 和格式检查、`pnpm run verify-notes`、`git diff --check`、`bash scripts/verify-local.sh --quick` 通过。

## 历史检索与参考

按 `useExternalUrl`、`noopener`、`popup blocked` 和外部链接检索活跃笔记，没有持有这条浏览器返回值决定的笔记；其他命中分别属于平台授权呈现和资源目录边界，与本问题无关。[MDN 的 `window.open` 文档](https://developer.mozilla.org/en-US/docs/Web/API/Window/open#noopener)说明启用 `noopener` 时返回 `null`。
