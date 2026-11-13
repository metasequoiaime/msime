# Agent Note: 纠错重排不把更长补全词当成完整输入

Status: implemented

## Problem

#11153 的 `yigyong` 在纯引擎中首选是「应用」，挂上随包句子模型后首选变成「应用程序」。补全词消费全部输入，但读音含用户未输入的音节。`candidate_answers_key` 把“选择后组字结束”当成“完整回答按键”，让补全词参与纠错重排。全拼含纠错时有意撤销词典首选的可信豁免，模型于是能把补全词抬到首位。

## Decision

生成普通组字快照时，词库行的完整读音若以匹配读音加音节分隔符开头，就认作更长的拼音补全，不标为 `candidate_answers_key`。组字消费、选择上屏、完整读音和学习字段保持原样；逐键和停顿重排共用该字段，不在宿主分别添加排序补丁。已有字段的文档本就排除超出输入的补全，不新增协议字段。

## Alternatives considered

- 所有纠错词典首选都恢复可信豁免：能固定「应用」，但也会禁止模型在纠错的多个合理读法之间选择，撤销现有全拼纠错设计，因此不采用。
- 直接按候选汉字数排短词优先：实现简单，但会误伤整句、混合文字和异切分；用词条已保留的匹配读音与完整读音判断实际补全边界。

## Verification

真实引擎与可控打分器回归在原实现下失败、修复后通过。覆盖纠错和精确输入的逐键、停顿重排，验证补全保留且按候选 ID 可显式上屏；另一打分器仍能把合理的纠错整词「英勇」升到首位。`msime-input-runtime` 全部 482 项测试、引擎库 3163 项测试（59 项既有 ignored）、31 项 golden 与两包 Clippy 通过。

锁定产品词库加随包 `sentence-model.safetensors` 重放：修前 `yigyong` 首选「应用程序」，修后首选「应用」；精确 `yingyong` 保持「应用」。golden 只把一个完整读音比匹配读音多一音节的补全行 `answers_key` 从 true 改成 false，不改输入、默认值或场景前提。

## Consequences

补全词不再参与完整输入的模型首选竞争，列表中的补全位置与显式选择仍由原有规则决定。纠错整词之间的模型竞争继续生效，避免把词典首选全部冻结。快照字段沿用文档原有含义；读取方只在候选变换和重排中使用它，选择结束状态仍由原有组字推进判断。尚未在报告者的真实编辑器安装验收。

## Related notes

[精确首位保留](../../implemented/bug-fix/2026-10-12-exact-reading-keeps-first-seat-over-longer-phrase.md) 与[模糊整词让替补整句](../../implemented/bug-fix/2026-10-08-fuzzy-whole-input-before-stand-in-sentence.md) 分别约束词典排序与模糊合并，本篇约束模型入口，部分重叠但不替代。

[纠错与简拼优先级](../../implemented/bug-fix/2026-10-29-quanpin-jianpin-over-autocorrect.md) 保留拼写解析规则；[重排文本缓冲](../../implemented/testing/2026-10-09-rerank-text-buffer.md) 保留模型上下文与预算，两者与本篇边界不同，均不替代。
