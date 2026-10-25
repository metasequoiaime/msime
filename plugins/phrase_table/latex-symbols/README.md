# latex-symbols：按 LaTeX 命令名打符号

K 模式的短语表（`kind = "phrase_table"`，纯数据，不带代码）：打 `Kalpha` 出 α，`Kinfty` 出 ∞，`Krightarrow` 出 →，`Kmathbbr` 出 ℝ。共 369 条，覆盖希腊字母、逻辑与集合、二元运算、关系、箭头、大型运算符、定界符、黑板粗体和常用的单位与文本符号。

| 项 | 值 |
| --- | --- |
| 类型 | 短语表（K 模式） |
| 许可证 | MIT（命令名和码位是事实，表是本项目自己写的） |
| 分发 | 随 macOS、Windows、Linux 的桌面安装包分发（`plugins/official-packs.json`），默认不启用；移动端安装包不带 |

## 用法

在「插件」里打开 LaTeX 符号（把它加进启用的短语表），确认「输入 › 快捷模式」里的 K 模式开着。没有输入时打大写 `K`，再打命令名：K 模式先列词库里的短语，再列编码以你打的字母开头的插件短语，所以 `Kal` 就能看到 α、ℵ 等。

只在桌面可用：短语表要在「插件」页启用，Android、iOS 和鸿蒙手机平板没有这个入口，鸿蒙电脑的键盘读不到设置应用导入的包（docs/plugins.md 开头的平台说明），移动端安装包也不带它。

## 编码规则

- 命令名去掉反斜杠和花括号、全部小写：`\alpha` 是 `alpha`，`\mathbb{R}` 是 `mathbbr`。短语表的编码只能是 1 到 32 个小写字母。
- 只差大小写的命令合在同一个编码下，小写的那个在前：`Kgamma` 出 γ、Γ，`Krightarrow` 出 →、⇒，`Kdelta` 出 δ、Δ。同一个编码最多 2 行。
- 字形按 LaTeX 排出来的样子取码位：`\phi` 是 ϕ，`\varphi` 是 φ；`\epsilon` 是 ϵ，`\varepsilon` 是 ε。
- 只收单个非 ASCII 字符：`\vert`、`\backslash` 这类键盘上直接能打的不收。命令名取自 LaTeX 内核、amssymb、amsmath、latexsym、stmaryrd、mathtools、gensymb 和 textcomp，数据不来自任何受版权保护的表（不用 unicode-math 的 LPPL 表，也不用 fcitx5 的 LGPL `latex.mb`）。

## 和内置符号面板的关系

内置符号面板（msime-dictionary `sources/symbols/piliapp-symbols.txt` 的「希腊字母」「数学符号」等分类，`crates/engine/src/local/catalog.rs`）能按分类浏览同一批符号。这个包补的只是「按名字查」：知道 LaTeX 命令名时，打 `Kalpha` 比翻分类快。

`symbols.tsv` 同时编进引擎，供符号模式（Shift+S）按完整命令名查符号：`Salpha`、`Ssum`、`Smathbbr` 在六个平台上都能用，不需要启用这个包（`crates/engine/src/local/symbol_groups.rs`）。符号模式用的编码规则与这里相同，改这张表会同时改变两处。这个包仍有用处：K 模式按前缀列出，打 `Kal` 就能看到所有以 `al` 开头的命令；符号模式只认完整的命令名。

## 生成和核对

`symbols.tsv` 每行「命令名<TAB>U+码位」，是 msime-dictionary `sources/symbols/latex-symbols.tsv` 的逐字节副本：加命令名、改码位都改词库仓那份，再更新 `resources/symbol-tables.lock.json` 并运行 `python3 scripts/sync_symbol_tables.py` 同步过来，`scripts/test-symbol-tables.py` 会拒绝在这里手改的副本。`pack/plugin.toml` 由生成器写出，不要手改：

```sh
python3 plugins/phrase_table/tools/gen_latex_symbols.py          # 重新生成 pack/plugin.toml
python3 plugins/phrase_table/tools/gen_latex_symbols.py --check  # 只核对，不写入
```

生成器只用标准库，在这些情况下失败：编码不是 1 到 32 个小写字母、命令名重复、码位是 ASCII 或控制、格式、私用、未分配的码位、同一个编码超过 2 行或者合并了两行却没有全小写的那个、同一个编码下两行是同一个字符、总条数不在 250 到 400 之间、`pack/LICENSE.txt` 不是 `plugins/phrase_table/LICENSE` 的副本。短语表本身的规则由 `msime-pack validate --builtin` 核对，`scripts/build-official-plugins.sh` 两样都跑，CI 的 Script plugins 作业跑它的 `--check`。

包文件夹是 `pack/`：只有 `plugin.toml` 和 `LICENSE.txt`，`scripts/build-official-plugins.sh` 把它原样放进 `target/plugin-packs/phrase_table/latex-symbols/`。
