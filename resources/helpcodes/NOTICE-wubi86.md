# 五笔 86 辅助码表：来源、许可与生成方法

## `wubi86_helpcode.txt`

| 项 | 值 |
| --- | --- |
| 方案标识 | `wubi86`（五笔 86 辅助码），由 `crates/engine/src/assets.rs` 的 `HELPCODES` 登记 |
| 内容 | 20614 个基本区汉字，每字一行 `字=前两码`，前面三行 `#` 开头的表头说明来源与规则 |
| 内容摘要 | `sha256:51b6f9d1c47ffa4dbaf24437d07eaec13e6bfd8ddcfb909c818e57471dfe4162`，20617 行（含表头），145068 字节 |
| 输入 | [metasequoiaime/msime-dictionary](https://github.com/metasequoiaime/msime-dictionary) 的 `sources/wubi/wubi86-jidian.txt`，`sha256:70afd8476e53b0f1b2184ebfafefb62212e8d3d554b12612cbae0336efa9a0e5`；生成时取自提交 `84b5239f98dfb5a4a8df8d92217aabb37cfa8db1`，该文件自 `134c358`（`dict-v2.0.6`）起没有变过 |
| 输入的上游 | [KyleBing/rime-wubi86-jidian](https://github.com/KyleBing/rime-wubi86-jidian)（Apache-2.0）；该文件另并入了 [rime/rime-wubi](https://github.com/rime/rime-wubi) 提交 `152a0d3f3efe40cae216d1e3b338242446848d07` 的词条（LGPL-3.0），见 msime-dictionary 的 `NOTICE.md` |
| 生成器 | `crates/dict-builder/src/wubi86_helpcode.rs`，随 [#6382](https://github.com/metasequoiaime/msime/pull/6382) 加入 |
| 生成命令 | `cargo run -p msime-dict-builder -- wubi86-helpcode --dictionary <msime-dictionary checkout> --out resources/helpcodes/wubi86_helpcode.txt` |

### 这张表是怎么来的

每个字取它在输入码表里单独出现时最长的编码（全码）的前两码，与 86 五笔词组补充表（`wubi86-supplement`）用的是同一个全码定义。一个字有几个最长编码、而它们的前两码不一致时不收，不替用户猜哪一种拆法：这样跳过了 317 个字，其中 GB2312 里的只有 43 个，是 41 个偏旁部首（它们在码表里另有 `copp`、`zzpp` 一类的部首查询编码）和「麴」「麸」。扩展 A 区和基本多文种平面以外的 49292 个字不收：`msime-wubi.db` 的 `wubi86` 表同样去掉了它们，它们不会出现在候选里。标点等非汉字的行不收。

本表是对输入码表的修改：只保留单字，并把编码截成两码。文件头写明了这一点（Apache-2.0 第 4 条 (b) 款）。表头只记录输入文件的 SHA-256 和规则，不记录任何提交，所以同一份输入在任何提交上都生成逐字节相同的文件；输入变了就重新运行上面的命令，提交新表，并更新本文件里的摘要和行数。

### 许可

输入码表的单字编码来自 KyleBing/rime-wubi86-jidian，按 Apache-2.0 提供，许可证全文附在本文件末尾；上游仓库没有 `NOTICE` 文件。

把输入文件与只含极点原始行的版本（msime-dictionary 提交 `0ccfa82` 之前的 `cn/Wubi86.txt`）逐字对照：两者都收的字，前两码全部相同；本表另有 16 个字只出现在并入的 rime-wubi 行里：U+6121（愡）、U+9FB4 至 U+9FBB 八个部件字，以及兼容表意文字区的 U+F92C、U+F979、U+F995、U+F9E7、U+F9F1、U+FA0C、U+FA0D（郎、凉、秊、裏、隣、兀、嗀的兼容写法）。这 16 行来自 LGPL-3.0 的数据。LGPL-3.0 是 GPL-3.0 加上附加许可，按 GPL-3.0 第 7 条，转发副本时可以去掉附加许可，因此这 16 行随本项目按 GPL-3.0 分发（全文见本项目的 `LICENSE`）。

### Apache License 2.0 全文

```text

                                 Apache License
                           Version 2.0, January 2004
                        http://www.apache.org/licenses/

   TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

   1. Definitions.

      "License" shall mean the terms and conditions for use, reproduction,
      and distribution as defined by Sections 1 through 9 of this document.

      "Licensor" shall mean the copyright owner or entity authorized by
      the copyright owner that is granting the License.

      "Legal Entity" shall mean the union of the acting entity and all
      other entities that control, are controlled by, or are under common
      control with that entity. For the purposes of this definition,
      "control" means (i) the power, direct or indirect, to cause the
      direction or management of such entity, whether by contract or
      otherwise, or (ii) ownership of fifty percent (50%) or more of the
      outstanding shares, or (iii) beneficial ownership of such entity.

      "You" (or "Your") shall mean an individual or Legal Entity
      exercising permissions granted by this License.

      "Source" form shall mean the preferred form for making modifications,
      including but not limited to software source code, documentation
      source, and configuration files.

      "Object" form shall mean any form resulting from mechanical
      transformation or translation of a Source form, including but
      not limited to compiled object code, generated documentation,
      and conversions to other media types.

      "Work" shall mean the work of authorship, whether in Source or
      Object form, made available under the License, as indicated by a
      copyright notice that is included in or attached to the work
      (an example is provided in the Appendix below).

      "Derivative Works" shall mean any work, whether in Source or Object
      form, that is based on (or derived from) the Work and for which the
      editorial revisions, annotations, elaborations, or other modifications
      represent, as a whole, an original work of authorship. For the purposes
      of this License, Derivative Works shall not include works that remain
      separable from, or merely link (or bind by name) to the interfaces of,
      the Work and Derivative Works thereof.

      "Contribution" shall mean any work of authorship, including
      the original version of the Work and any modifications or additions
      to that Work or Derivative Works thereof, that is intentionally
      submitted to Licensor for inclusion in the Work by the copyright owner
      or by an individual or Legal Entity authorized to submit on behalf of
      the copyright owner. For the purposes of this definition, "submitted"
      means any form of electronic, verbal, or written communication sent
      to the Licensor or its representatives, including but not limited to
      communication on electronic mailing lists, source code control systems,
      and issue tracking systems that are managed by, or on behalf of, the
      Licensor for the purpose of discussing and improving the Work, but
      excluding communication that is conspicuously marked or otherwise
      designated in writing by the copyright owner as "Not a Contribution."

      "Contributor" shall mean Licensor and any individual or Legal Entity
      on behalf of whom a Contribution has been received by Licensor and
      subsequently incorporated within the Work.

   2. Grant of Copyright License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      copyright license to reproduce, prepare Derivative Works of,
      publicly display, publicly perform, sublicense, and distribute the
      Work and such Derivative Works in Source or Object form.

   3. Grant of Patent License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      (except as stated in this section) patent license to make, have made,
      use, offer to sell, sell, import, and otherwise transfer the Work,
      where such license applies only to those patent claims licensable
      by such Contributor that are necessarily infringed by their
      Contribution(s) alone or by combination of their Contribution(s)
      with the Work to which such Contribution(s) was submitted. If You
      institute patent litigation against any entity (including a
      cross-claim or counterclaim in a lawsuit) alleging that the Work
      or a Contribution incorporated within the Work constitutes direct
      or contributory patent infringement, then any patent licenses
      granted to You under this License for that Work shall terminate
      as of the date such litigation is filed.

   4. Redistribution. You may reproduce and distribute copies of the
      Work or Derivative Works thereof in any medium, with or without
      modifications, and in Source or Object form, provided that You
      meet the following conditions:

      (a) You must give any other recipients of the Work or
          Derivative Works a copy of this License; and

      (b) You must cause any modified files to carry prominent notices
          stating that You changed the files; and

      (c) You must retain, in the Source form of any Derivative Works
          that You distribute, all copyright, patent, trademark, and
          attribution notices from the Source form of the Work,
          excluding those notices that do not pertain to any part of
          the Derivative Works; and

      (d) If the Work includes a "NOTICE" text file as part of its
          distribution, then any Derivative Works that You distribute must
          include a readable copy of the attribution notices contained
          within such NOTICE file, excluding those notices that do not
          pertain to any part of the Derivative Works, in at least one
          of the following places: within a NOTICE text file distributed
          as part of the Derivative Works; within the Source form or
          documentation, if provided along with the Derivative Works; or,
          within a display generated by the Derivative Works, if and
          wherever such third-party notices normally appear. The contents
          of the NOTICE file are for informational purposes only and
          do not modify the License. You may add Your own attribution
          notices within Derivative Works that You distribute, alongside
          or as an addendum to the NOTICE text from the Work, provided
          that such additional attribution notices cannot be construed
          as modifying the License.

      You may add Your own copyright statement to Your modifications and
      may provide additional or different license terms and conditions
      for use, reproduction, or distribution of Your modifications, or
      for any such Derivative Works as a whole, provided Your use,
      reproduction, and distribution of the Work otherwise complies with
      the conditions stated in this License.

   5. Submission of Contributions. Unless You explicitly state otherwise,
      any Contribution intentionally submitted for inclusion in the Work
      by You to the Licensor shall be under the terms and conditions of
      this License, without any additional terms or conditions.
      Notwithstanding the above, nothing herein shall supersede or modify
      the terms of any separate license agreement you may have executed
      with Licensor regarding such Contributions.

   6. Trademarks. This License does not grant permission to use the trade
      names, trademarks, service marks, or product names of the Licensor,
      except as required for reasonable and customary use in describing the
      origin of the Work and reproducing the content of the NOTICE file.

   7. Disclaimer of Warranty. Unless required by applicable law or
      agreed to in writing, Licensor provides the Work (and each
      Contributor provides its Contributions) on an "AS IS" BASIS,
      WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
      implied, including, without limitation, any warranties or conditions
      of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or FITNESS FOR A
      PARTICULAR PURPOSE. You are solely responsible for determining the
      appropriateness of using or redistributing the Work and assume any
      risks associated with Your exercise of permissions under this License.

   8. Limitation of Liability. In no event and under no legal theory,
      whether in tort (including negligence), contract, or otherwise,
      unless required by applicable law (such as deliberate and grossly
      negligent acts) or agreed to in writing, shall any Contributor be
      liable to You for damages, including any direct, indirect, special,
      incidental, or consequential damages of any character arising as a
      result of this License or out of the use or inability to use the
      Work (including but not limited to damages for loss of goodwill,
      work stoppage, computer failure or malfunction, or any and all
      other commercial damages or losses), even if such Contributor
      has been advised of the possibility of such damages.

   9. Accepting Warranty or Additional Liability. While redistributing
      the Work or Derivative Works thereof, You may choose to offer,
      and charge a fee for, acceptance of support, warranty, indemnity,
      or other liability obligations and/or rights consistent with this
      License. However, in accepting such obligations, You may act only
      on Your own behalf and on Your sole responsibility, not on behalf
      of any other Contributor, and only if You agree to indemnify,
      defend, and hold each Contributor harmless for any liability
      incurred by, or claims asserted against, such Contributor by reason
      of your accepting any such warranty or additional liability.

   END OF TERMS AND CONDITIONS

   APPENDIX: How to apply the Apache License to your work.

      To apply the Apache License to your work, attach the following
      boilerplate notice, with the fields enclosed by brackets "[]"
      replaced with your own identifying information. (Don't include
      the brackets!)  The text should be enclosed in the appropriate
      comment syntax for the file format. We also recommend that a
      file or class name and description of purpose be included on the
      same "printed page" as the copyright notice for easier
      identification within third-party archives.

   Copyright [yyyy] [name of copyright owner]

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
```
