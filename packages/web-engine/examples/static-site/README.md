# 静态站点示例

一个只有一页的站点：选方案、在文本框里打字。`npm run build` 用 `msime-web-engine copy` 把 SDK 和资源复制到 `site/msime/`，`site/` 就是要发布的目录。

```sh
npm install
npm run build
npx serve site        # 本地预览，打开 http://localhost:3000
```

## 部署

| 平台 | 步骤 |
| --- | --- |
| GitHub Pages | 把 `github-pages.yml` 复制到仓库的 `.github/workflows/`，仓库设置的 Pages 选 GitHub Actions，推送到 main。 |
| Vercel | 导入仓库（Root Directory 选这个目录），`vercel.json` 已经写好构建命令和输出目录。也可以 `npx vercel`。 |
| Cloudflare Pages | 构建命令 `npm run build`，输出目录 `site`。`site/_headers` 配置缓存。 |
| Cloudflare Workers | `npm run build && npx wrangler deploy`，`wrangler.jsonc` 用 Static Assets 托管 `site/`。 |

不想部署资源文件时，把 `site/index.html` 里的 `./msime/index.js` 换成 `https://cdn.jsdelivr.net/npm/@msime/web-engine@<版本>/index.js`，`npm run build` 那一步也可以省掉。
