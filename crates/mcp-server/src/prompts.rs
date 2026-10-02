//! Prompts: canned requests an assistant offers its user, such as slash commands in Claude Code. Each is a set of instructions to the assistant that walks it through one task with this server's tools.

use crate::server::MsimeServer;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{PromptMessage, Role};
use rmcp::schemars::JsonSchema;
use rmcp::{prompt, prompt_router};
use serde::Deserialize;

/// The prompts that need a tool only `--allow-write` offers.
pub const WRITE_PROMPTS: [&str; 1] = ["make_skin"];

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct MakeSkinArgs {
    /// What the skin should look like, in the user's words: colours, a mood, light or dark, a picture to use. Leave empty to be asked.
    pub style: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct DiagnoseArgs {
    /// What goes wrong, in the user's words. Leave empty to be asked.
    pub problem: Option<String>,
}

/// The argument as the user gave it, or `None` when it is missing or blank: a client may send an empty string for an argument the user skipped.
fn given(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

pub fn make_skin_text(style: Option<String>) -> String {
    let request = match given(style) {
        Some(style) => format!("The user wants this skin: {style}\n\nAsk only about what this leaves open and you cannot reasonably choose yourself."),
        None => "Ask the user in one short question what they want: colours or a mood, light, dark or both, and whether they want a background or decoration image. Choose everything else yourself.".to_owned(),
    };
    format!(
        "Make a candidate-window skin for 水杉输入法 (MSIME) and install it with create_candidate_skin. Talk to the user in their own language.

{request}

1. Call list_candidate_skins and pick a package_id no installed skin uses, unless the user wants to replace one of theirs.
2. Write skin.toml as create_candidate_skin's description lays out. Give [candidate.light] and [candidate.dark] colours for each mode listed in [supports] themes, and keep text and number readable against surface and selected (a contrast of at least 4.5:1).
3. Make every image by running code; never write base64 by hand. A short Python script using only the standard library (zlib and struct are enough to write a PNG) runs without installing anything; use Pillow only if it is already installed. Always make the preview, a small picture of the skin in its own colours, around 480x160 and under 256 KiB. Keep background and decoration images to a few hundred pixels; at most 3 images and 2 MiB together. Have the script print each image's base64 and pass it in images, keyed by the path the manifest uses. If you cannot run code, tell the user that making a skin needs an assistant that can, such as Claude Code or Codex, and stop.
4. If create_candidate_skin refuses the skin, read the reason, fix the manifest or the images, and try again.
5. Once it is installed, tell the user how to use it: in the MSIME settings, open 主题 and pick the skin under 我的皮肤. If they are signed in, the desktop app also saves it to their cloud library as a private skin, and they can publish it from its card there."
    )
}

pub fn diagnose_text(problem: Option<String>) -> String {
    let request = match given(problem) {
        Some(problem) => format!("The user reports: {problem}"),
        None => "Ask the user in one short question what goes wrong and when.".to_owned(),
    };
    format!(
        "Help the user with a problem in 水杉输入法 (MSIME). Talk to them in their own language and in plain words; they may not be technical, so do every step yourself instead of asking them to open files, settings or a terminal.

{request}

1. Read the log with read_diagnostic_log. If logging is off, turn it on with set_diagnostic_log, ask the user to do again what went wrong and to tell you when they have, then read the log again.
2. Explain what happened and what to do about it. If the log does not show the cause, say so rather than guessing.
3. Turn logging off with set_diagnostic_log when you are done."
    )
}

#[prompt_router(vis = "pub(crate)")]
impl MsimeServer {
    #[prompt(
        name = "make_skin",
        description = "Design a candidate-window skin with the user and install it."
    )]
    async fn make_skin(&self, Parameters(args): Parameters<MakeSkinArgs>) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            Role::User,
            make_skin_text(args.style),
        )]
    }

    #[prompt(
        name = "diagnose",
        description = "Find out why the input method misbehaves, using its diagnostic log."
    )]
    async fn diagnose(&self, Parameters(args): Parameters<DiagnoseArgs>) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            Role::User,
            diagnose_text(args.problem),
        )]
    }
}
