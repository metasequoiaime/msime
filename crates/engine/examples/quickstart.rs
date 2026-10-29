// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! 快速上手：在一个数据目录上准备运行时，用引擎库的默认选项建会话，打一句全拼，列出候选，选第一个上屏，写下学习，干净地结束。
//!
//! ```text
//! cargo run --release --example quickstart -- <数据目录> [<工作目录>]
//! ```
//!
//! 数据目录至少要有 `msime-pinyin.db` 和 `msime-english.db`，再有 `msime-bigram.bin`、`msime-trigram.bin` 时整句排序与 msime 自己的相同；msime-engine-capi 发布里的数据包 `msime-engine-data-quanpin-*` 解开后就是这样一个目录。工作目录放学习日志、准备好的工作词库和缓存，缺省是系统临时目录下的 `msime-quickstart`。
//!
//! 只用 crate 根上有文档的核心 API，不读仓库里的任何文件，所以这个示例随 crates.io 的包发布。

use std::path::Path;

use msime_engine::{prepare_runtime_paths, DictionaryAccess, Session, SessionOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let data = arguments
        .next()
        .ok_or("usage: quickstart <data directory> [<work directory>]")?;
    // 四个根目录都要是绝对路径，`resources`、`user_data`、`cache` 两两不重叠。
    let resources = std::path::absolute(data)?;
    let work = match arguments.next() {
        Some(path) => std::path::absolute(path)?,
        None => std::env::temp_dir().join("msime-quickstart"),
    };

    // 准备代次：在 `user_data/dictionaries/<content id>` 里复制工作词库、回放学习日志；同一个 id 再调只补回放。交回的 `RuntimePaths` 就是建会话要的四个目录。
    let paths = prepare_runtime_paths(
        &resources,
        &work.join("user"),
        &work.join("cache"),
        &content_id(&resources),
    )?;
    // 会话期间持共享租约：同一份用户数据上有人（本进程、别的进程，包括 msime 自己的输入法）正在改个人词典或重置学习时，不开会话。
    let access = DictionaryAccess::try_session(&paths.user_data, &paths.dictionaries)?
        .ok_or("the dictionaries are being maintained; try again later")?;
    // `SessionOptions::new` 是引擎库的默认值：全拼、小鹤双拼、学习开、中文标点开。
    let mut session = Session::new(SessionOptions::new(paths))?;

    for key in b"nihao" {
        session.character(*key, false);
    }
    // `display_snapshot` 是给用户看的状态：全拼的组字按音节分开（`ni'hao`）。
    let state = session.display_snapshot();
    println!("preedit: {}", state.preedit);
    for (index, candidate) in state.candidates.iter().take(9).enumerate() {
        println!("{}. {}", index + 1, candidate.word);
    }

    let result = session.select(0);
    match result.commit {
        Some(text) => println!("commit: {text}"),
        None => println!("still composing: {}", session.display_snapshot().preedit),
    }
    // 学习没能存下之类的诊断：键照样处理了，只是告诉宿主。
    if let Some(diagnostic) = result.diagnostic {
        eprintln!("diagnostic: {diagnostic}");
    }

    // 结束会话：写下它排着的学习、交还日志连接，然后放掉租约。
    drop(session);
    drop(access);
    // 退出或卸载前：写完所有排着的个人上下文学习（平时延迟约 2 秒写），停掉后台写线程。
    msime_engine::shutdown();
    Ok(())
}

/// 代次 id 是 1 到 128 个 `[0-9A-Za-z_-]`，数据一换就要换一个，否则会继续用旧数据准备好的代次。这里取数据目录的名字（数据包解开后是 `msime-engine-data-quanpin-2.0.41` 这样带版本号的名字），其余字符换成 `_`。
fn content_id(resources: &Path) -> String {
    let name = resources
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let id: String = name
        .chars()
        .take(128)
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if id.is_empty() {
        "data".to_owned()
    } else {
        id
    }
}
