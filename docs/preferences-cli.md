# 用文件配置偏好

`msime-mcp`（也可通过 `msime` 链接运行）的 `config schema`、`revision`、`validate` 和 `apply` 面向 dotfiles 与脚本。它们覆盖共享 Rust 偏好的全部字段，包括设置页没有控件的字段；原有 `config get/set` 仍用于少量常用设置。

这些命令使用 JSON，不改变六个平台的 `preferences.json` 存储格式。桌面安装包提供此可执行文件；移动端仍由原生设置应用读写同一套共享偏好，手机应用内不增加终端入口。

## 查键名与默认值

```sh
msime-mcp config schema > preferences.schema.json
```

Schema 直接从当前版本的 Rust/serde 类型生成。顶层 `properties` 列出字段，`$defs` 列出子对象与枚举类型，数组的 `items` 给出元素类型；默认不落盘的可选字段也会出现。`description` 来自源码文档注释；默认值是该二进制构建的 full 版出厂值，并不代表当前用户值。其他 edition 和升级前已准备的目录可能有不同默认值。

Schema 描述完整偏好对象，不包含外层 `format_version`、`revision`、`preferences` 信封，也不是 Merge Patch 的 Schema。Schema 用于查键名和编辑器提示，最终业务校验以 `config validate` 和应用事务为准；范围、跨字段关系以及已有的兼容读法没有复制到 Schema 里。注释没有明确说明的平台支持或设置页入口，不应由字段存在推断。

## 预检并应用

将需要修改的字段写成 JSON Merge Patch，例如 `preferences.patch.json`：

```json
{
  "candidate_page_size": 7,
  "default_ime_mode": "english",
  "sentence_association": {
    "sentence_collocation_weight": 2.0
  }
}
```

指定输入法的实际状态目录，预检不会创建目录、锁文件或备份，也不会恢复损坏文档：

```sh
msime-mcp --state-dir /absolute/path/to/state config validate /absolute/path/to/preferences.patch.json
```

成功返回 `{"valid":true,"revision":N}`。也可用 `config revision` 只读取当前 revision；这两个命令都不输出偏好值。确认预检结果后，把返回的 revision 传给应用命令：

```sh
msime-mcp --state-dir /absolute/path/to/state --allow-write config apply /absolute/path/to/preferences.patch.json 0
```

例子中的 `0` 必须替换为预检实际返回的数值。应用成功返回 `{"applied":true,"revision":N}`，在偏好锁内重新读取、校验、比较 revision 并原子保存。期间设置页或另一个脚本修改过配置就拒绝应用；重新检查实际配置与补丁后再预检，不要自动用新 revision 重试覆盖。

直接在终端里亲自运行 `config apply` 与原有 `config set` 一样允许写入；被程序或助手调用时必须显式提供 `--allow-write`。这些文件命令没有增加任何 MCP 工具，也不扩大原有 MCP `update_preferences` 的字段白名单。

- 对象递归合并，只改列出的键；数组整体替换。
- `null` 删除字段，删除后必须仍能按现有默认/必填规则解码，不保证每个字段都能删除。
- 新增或更改本版本不认识的键会拒绝；磁盘上已有且未变化的未来字段继续保留。
- 校验和应用使用现有共享偏好业务规则及兼容读法，不另设一套默认值。
- 输入必须是单链接的普通 UTF-8 JSON 文件，上限 1 MiB；不读取符号链接、目录或 FIFO。
- 成功退出码为 `0`，文档/权限/冲突错误为 `1`，命令用法错误为 `2`。失败时不输出偏好或文件内容。

省略 `--state-dir` 时，命令沿用原有 `--options`、环境变量和平台默认位置定位目录。显式指定状态目录时无需先安装或打开设置页；目录中的 edition 记录和既有缺文件默认规则仍然生效。
