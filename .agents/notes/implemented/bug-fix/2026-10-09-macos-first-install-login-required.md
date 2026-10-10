# Agent Note: macOS 首次安装后，这次登录看不到输入法时一路都提示重新登录

Status: implemented

## Problem

#6280：用户在 Mac 上第一次安装，从 DMG 把设置应用拖进「应用程序」，打开后点「立即安装」。bundle 已经在 `~/Library/Input Methods` 里，系统设置「添加」对话框里却找不到水杉输入法。用户没有附日志，报的系统版本「macOS 16」并不存在。

代码里已经知道 macOS 的一条限制（platforms/macos/README.md「安装与输入源注册」）：一个在本次登录开始时不在输入源列表里的 bundle identifier，`TISRegisterInputSource` 返回 noErr，但要到下次登录才进注册表和「添加」对话框。全新机器第一次安装就是这样：`--register-input-source` 登记后查不到源，返回 fnfErr、退出码 1，设置应用把它当成 `RegistrationPending`，报 `login_required`。问题出在这之后的提示链路，有四处会把用户引错方向：

- 安装窗口对 `login_required` 的标题仍是「安装完成」，「要注销并重新登录」只写在副标题里。用户看到「完成」就去系统设置找，找不到就以为没装上。
- 同一次登录里再打开设置应用，已装版本等于内嵌版本，启动检查只能报 `up_to_date`。状态请求只重读 `enabled`，于是设置页把提示换成「把水杉输入法加入输入法列表」和添加步骤，让用户去添加一个这次登录根本列不出来的输入法。
- DMG 里的 `安装说明.txt` 没提首次安装要重新登录。
- `install_unlocked` 用 `?` 接 `install_bundle_at_with_registration` 的结果，`RegistrationPending` 时提前返回，跳过了 `stop_running_copies` 和 `refresh_system_input_source_lists`。README 还记着另一种情况：刚登记后有一段分钟级的注册表空窗，`TISCreateInputSourceList` 会暂时查不到源。如果 fnfErr 来自这段空窗，源过一会儿就进了注册表，但「添加」对话框读的是键盘设置扩展在登录时写下的 `com.apple.IntlDataCache.le*` 缓存。不清掉这份缓存，就要等到下次登录对话框才列得出来。

## Decision

### 只读的注册表探针

输入法可执行文件新增参数 `--input-source-registered`（`MSIMEShouldReportInputSourceRegistration`）。它用 `MSIMEInputSourceRegistryStateFor` 按登记后那次查找的同一个条件查注册表：bundle identifier、`kTISPropertyInputSourceIsEnableCapable`、`includeAllInstalled = true`。NULL 和空列表都算 Missing，和登记返回 fnfErr 的条件一致。这个参数不登记、不启用任何东西。退出码由 `MSIMEInputSourceRegistryExitCode` 决定：Listed 退 0，Missing 退 3，缺 identifier 或查询函数时是 Unknown，退 1。查询函数是注入的，`input-source-registration` 这项 CTest 用替身 lister 覆盖三种状态，并确认探针不调用 enabler。

### 设置应用怎样用这个探针

- `macos_input_source::input_source_registered()` 先用 `validate_bundle` 检查已装的 bundle，再带上这个参数跑它的可执行文件。退出码 0 和 3 映射成 `Some(true)` / `Some(false)`，其余退出码、起不来、被信号结束都是 `None`。每次最多等 5 秒，超时就杀掉进程：一个不认识这个参数的旧版输入法会当成普通启动，起 IMK 服务后一直运行下去。探针最多查 3 次，间隔 1 秒，每次都查不到才算 `Some(false)`；任何一次查到或无从判断就立即返回。
- `input_source_status_now` 多一个注入的 `registered` 参数。只有启动结果是 `up_to_date` 且 `enabled == Some(false)` 时才调用它；返回 `Some(false)` 就把这次回答的 action 改成 `login_required`，存着的启动结果不改。设置页对 `login_required` 本来就有「重新登录后才能添加水杉输入法」的提示，前端逻辑不变。
- `installed`、`updated` 不调用探针：这次启动刚替换过 bundle，正处在登记后的空窗里，查不到不说明什么。`enabled` 为 `None`（列表读不到）时也不调用。
- 查明的结果（`Some`）存进 `InputSourceStartupState::registered`，本次运行只查一次。设置页等用户添加时每 3 秒问一次状态，不能每次都拉起输入法。`finish` 换了启动结果时清掉这个缓存。`None` 不缓存，下次询问再查。
- 查询由单独的 `registry_probe` 锁串行：设置页挂载时的 `refresh()` 和窗口 `focus` 几乎同时发请求，不串行的话两个请求都看到缓存为空，各拉起最多 3 次输入法。后到的请求等前一个查完，直接用它存下的结论。查询期间不拿着 `registered` 本身，否则 `finish` 要等一次最长十几秒的查询。
- 缓存带一个代数，`finish` 每次加一。查询开始前记下代数，写回时不一致就丢掉结论：查询期间安装窗口换了启动结果，旧结论不能挂到新结果上。

### 同一次登录里的更新也报 `login_required`

首次安装报了 `login_required`、用户没注销，又打开了更新版 DMG 里的设置应用：启动检查判定 Update，`--register-input-source` 仍因标识符不在这次登录的注册表里返回 fnfErr。原来的逻辑因为有旧版本就回滚、报 `failed`，设置页叫用户点「安装 / 更新」重试，重试同样失败，这一整次登录里每次启动都这样，用户始终看不到「重新登录」这个真正的解法。

现在 `install_bundle_at_with_registration` 多一个注入的 `listed_before`。有旧版本时，替换之前用设置应用内嵌那份 bundle 跑一次 `--input-source-registered`（`registry_lookup`，不重试）。查的是标识符，哪份 bundle 跑结果都一样，而且这时还没登记，不在空窗里。登记失败而替换前查到的是 `Some(false)`：旧版本这次登录同样列不出来，回滚换不来任何东西，于是留下新版本、删掉备份，返回 `RegistrationPending`，启动检查据此报 `login_required`，`after_install_refreshes` 照样刷新系统列表。查到在注册表里或无从判断（旧的输入法、跑不起来、超时）时照旧回滚、报失败。首次安装没有旧版本，不调用它。

### `RegistrationPending` 也刷新系统列表

`install_unlocked` 用 `after_install_refreshes` 判断：成功或 `RegistrationPending` 时，都执行 `stop_running_copies` 和 `refresh_system_input_source_lists`（清缓存、退出系统设置、结束切换器进程），然后照原样返回结果。其他错误路径已经恢复了旧 bundle，不刷新。

### 文案

- 安装窗口对 `login_required` 的标题是「还差一步：注销并重新登录」。副标题说明输入法已复制到本机，但 macOS 要到下次登录才会列出它。按钮仍是「进入设置」，设置页顶部有同样的提示。
- `安装说明.txt` 加一段：如果安装窗口提示要重新登录，或在添加列表里找不到输入法，就从苹果菜单「退出登录」，重新登录后再去添加。
- platforms/macos/README.md「发布包」一节和 `--register-input-source` 那段同步说明这些行为。

## Alternatives considered

- **在 Rust 里经 FFI 链接 Carbon，直接调 `TISCreateInputSourceList`**：不用每次拉起一个进程，也没有旧版输入法不认参数的问题。但设置应用现在不链接 Carbon，还要在 Rust 里管 CF 对象的生命周期。注册表判定已经在输入法的 `InputSourceRegistration.mm` 里，有注入测试，加一个只读参数改动更小，判定条件和登记后的查找也只有一份。
- **对 `installed` / `updated` 也查注册表**：能多覆盖「这次启动刚装好、登记却查不到」的情况。但那段分钟级的注册表空窗正好落在这次启动里，误报的 `login_required` 会让一个其实能直接添加的用户去注销。首次安装查不到时已经由 `RegistrationPending` 报 `login_required`，不需要探针。
- **`login_required` 之后持续探测，源一出现就改回「去添加」**：如果 fnfErr 来自空窗，这样能少一次登录。但「添加」对话框读的是缓存：用户在源出现之前打开过系统设置，扩展就会把不含水杉的列表重新写进缓存，这时改口叫用户去添加，用户还是找不到。重新登录对两种成因都有效，所以 `login_required` 一旦报出，本次运行内不再撤回。
- **更新登记失败后再查注册表**：评审建议「替换前后都查不到」才算这种情况。替换后的查询正落在登记后的空窗里，查不到不说明什么；替换前那一次已经足够说明旧版本这次登录列不出来，所以只查替换前。
- **整个查询期间拿着 `registered` 一把锁**：最简单，但 `finish` 会被一次最长十几秒的查询卡住，安装窗口的「立即安装」要跟着等。改用单独的查询锁加代数。
- **只缓存「在注册表里」，「不在」每次重查**：不在注册表里的情况在本登录会话内不会自己变好（新 identifier 要等下次登录），每次重查只是每次多拉起 3 次进程、多等 2 秒。两种结论都缓存。

## Consequences

- **收益**：首次安装后，安装窗口和同一次登录里之后的每次启动都提示重新登录，不会再让用户去找一个这次登录列不出来的输入法。`安装说明.txt` 也写了这一步。fnfErr 来自注册表空窗时，「添加」对话框的缓存已经清掉，不必等下次登录。
- **代价**：`up_to_date` 且输入法不在列表里时，本次运行第一次状态请求要拉起一次输入法可执行文件。不在注册表里时最多拉起 3 次，多等约 2 秒。每次更新（启动检查和「安装 / 更新」按钮）在替换前多拉起一次内嵌的输入法，正常不到一秒，上限 5 秒。
- **已知上限**：
  - 一个登录会话里的第一次启动因空窗报了 `login_required`，用户又在源出现之前打开过系统设置，之后同一次登录里再启动时，探针会查到源、改回「去添加」，而对话框的缓存已经重写成不含水杉的版本。这种情况只能靠重新登录解决，设置应用没有跨启动记住「本登录会话等过登录」。
  - 用户若已经重新登录过仍然找不到，根因就不是这条限制，这次改动只让提示更准确。可能的原因有 `/Library/Input Methods` 里的冲突副本（设置页已单独提示）或别的缓存问题，需要用户反馈来确认。
  - 「新 identifier 要等下次登录」这条只在 README 记录的那台机器上实测过，没记 macOS 版本，还没在用户的系统上验证。

## Verification

- 原生：`ctest --test-dir target/macos-isolated -R '^input-source-registration$'`，覆盖参数识别、Listed / Missing / Unknown 三种状态及其退出码，并确认探针用的过滤条件与登记相同、不调用 enabler。
- 端到端（Studio 上，经 SSH）：构建出的 `水杉输入法.app` 带 `--input-source-registered` 运行，在已登记该 identifier 的账户上退 0。把副本的 `CFBundleIdentifier` 改成合成值并 ad-hoc 重签后运行，退 3。
- Rust：`cargo test -p msime-desktop --lib`。`macos_input_source` 的测试覆盖 `after_install_refreshes`、退出码映射、重试、探针传参和超时杀进程，以及更新时替换前查到不在注册表里就留下新版本报 `RegistrationPending`、查到在或无从判断时照旧回滚、首次安装不查。`tests.rs` 覆盖 `up_to_date` 加 `enabled == Some(false)` 时的 `login_required`、只查一次、`None` 不缓存、`installed` / `updated` 不查、`finish` 后重查、并发请求只查一次、查询期间 `finish` 时旧结论不进缓存。
- 前端：`apps/desktop/tests/settings/macos-install-page.test.tsx` 断言 `login_required` 时标题是「还差一步：注销并重新登录」，页面上没有「安装完成」。
- 还没在真机上走过完整流程：用一个本次登录从未见过该 identifier 的新账户安装 DMG、重开设置应用、注销再登录。
