# Agent Note: 个人词库同步遇到活动会话时保留重试

Status: implemented

## Problem

命名词库把词条送进个人词库队列后，键盘空闲同步会调用 `msime_client_personal_dictionary_sync`。原实现没有在同步批次外持有 `DictionaryAccess` 的维护租约，而是让每个词条单独调用引擎编辑。活动输入会话持有共享租约时，首批词条得到 `dictionary maintenance busy`，`PersonalDictionaryStore::synchronize_reporting_present` 却把任意编辑错误记成 `Failed`。失败请求不会被命名词库的 flush 自动重新送出，导入词条因此永久停在未生效状态。

## Decision

`personal_dictionary_sync_json` 在读取队列前取得 `DictionaryAccess::try_maintenance`，把整次有界同步保持在同一个独占窗口内。活动会话或其他维护者存在时立即返回稳定的 `dictionary maintenance busy`，队列状态保持 `Pending`。批内编辑调用新增的 `edit_personal_dictionary_with_access`，复用原有的词库种类、前后条目、权重和请求 ID 校验，但不重复申请已经持有的维护锁；原来的单条编辑入口仍自行取得租约。

同步仍每轮最多应用四条，Android 的键盘启动和收起时机不变；本修复只改变锁竞争的失败语义，不保证导入后首次弹出键盘时整份词库已经写完。

## Alternatives considered

- **只把 `dictionary maintenance busy` 改成 Pending。** 能避免这次失败，但批次中的其他编辑仍分别取锁，活动会话结束和下一条编辑之间仍有竞态，不能保证一个同步窗口内的结果一致。
- **同步时销毁或重建输入会话。** 可以让维护拿到锁，但会打断用户正在编辑的文本，违反宿主会话边界。
- **每个词条增加重试。** 能暂时吸收竞争，却延长工作线程占用并留下批次中途失败的状态；一次有界维护租约更容易保持原子边界。

## Consequences

同步遇到活动会话时会快速失败，待处理词条保留为可重试状态；会话销毁后下一轮同步可继续处理。同步批次内共享一把维护租约，减少重复开关锁的竞态。错误条目本身仍按原规则进入 `Failed`，只有锁竞争在批次开始前被区分出来。

## Verification

- 新增 host-api 合成用例：活动会话加六个合成拼音词时，同步返回 `dictionary maintenance busy`，六条仍为 `Pending`；销毁会话后两轮同步全部变为 `Applied`。
- 同一批新增的合成 SCEL 端到端用例确认完整同步后，26 键和九键都能查到所有合成词条。
- 真实 issue 附件解析为 74 条，目标读音在第 6 条，未触及两万条集合上限；因此本修复不把 SCEL 解析或九键查询宣称为根因。
