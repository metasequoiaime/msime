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
2. Write skin.toml as create_candidate_skin's description lays out. Give [candidate.light] and [candidate.dark] colours for each mode listed in [supports] themes, and keep text and number readable against surface and selected (a contrast of at least 4.5:1). Write colours as #RRGGBB or #RRGGBBAA, which every host reads.
3. Use schema_version = 2 for a lively skin. Animation and decoration motion work with schema_version = 2 on any base, so keep base = system or a built-in theme for an animated skin. Add an [animation] table that writes all three presets explicitly, a short enter (fade_scale or slide_up), exit (fade) and selection (spring or glow), with duration_ms around 160-220 and stagger_ms around 10-18; macOS plays only the presets the table writes. The native hosts map these presets to their own compositor; reduced-motion settings still win. Use schema_version = 1 only when the user explicitly asks for a static skin.
4. With a decoration image in [candidate_window.decoration] (singular: a character or ornament standing on the card's top edge), you can make it move while the candidate window is shown: set motion in [candidate_window.decoration] to float, sway, breathe or bounce (the host keeps the movement small) and optionally motion_period_ms (1200-8000, default 3000; around 2400-4000 feels calm). For frame animation, make the decoration an animated PNG (APNG): a few to a few dozen small frames (at most 120, each frame counts as a whole image toward the 8,000,000-pixel limit), a frame delay of about 80-200 ms, and a first frame that looks right on its own, because hosts that cannot play frames, and every host when the user asks the system to reduce motion, show only the first frame. Motion and APNG can be combined. Leave motion out, or use a still PNG, when the user wants a calm skin.
5. Use base = \"canvas\" only when the user explicitly asks for an msime-windows full-image (nine-slice) skin. It needs schema_version = 2 (version 1 with canvas is refused); give it a [candidate_window.border_image] whose images can be PNG/JPEG/APNG/GIF/WebP. border_image, [[candidate_window.decorations]] (plural: free widgets) and the canvas insets are read only with base = canvas. The Web engine and the macOS, Windows, iOS, Android and HarmonyOS candidate windows draw these canvas textures using their platform image codecs, while a host without that renderer (Linux for now) uses a readable opaque canvas look (only accent and menu colours are kept in that fallback). Explain the target host's support before making the skin. [animation] and decoration motion work on canvas as on any other base. The preview, panel, widgets and toolbar images count toward the 64-image local limit; community uploads remain limited to 37 images. canvas and wechat_scroll are reserved and cannot be a package_id. Extended resources and new bases require explicit server support before upload; older deployments keep these skins local.
6. Make every image by running code; never write base64 by hand. A short Python script using only the standard library (zlib and struct are enough to write a PNG, and an APNG is the same file with an acTL chunk and one fcTL plus fdAT pair per extra frame) runs without installing anything; use Pillow only if it is already installed. Use PNG/JPEG/APNG/GIF/WebP for resources and PNG/JPEG for the preview. Always make the preview, a small picture of the skin in its own colours, around 480x160 and under 256 KiB. Keep background and decoration images to a few hundred pixels; at most 64 images and 2 MiB together for local installation (community uploads remain limited to 37 and image toolbars require explicit server capability). Have the script save each image as a file in a new folder of its own and pass the files' absolute paths in image_files, keyed by the path the manifest uses; do not copy base64 into the call, where a single wrong character gets the whole skin refused. Only if you cannot write files, have the script print each image's base64 and pass it in images. If you cannot run code, tell the user that making a skin needs an assistant that can, such as Claude Code or Codex, and stop.
For an image toolbar requested by the user, follow the toolbar image contract in create_candidate_skin: make its background and optional state/button images, use button_defaults and button_flow for automatic placement, and include every toolbar image in image_files. Preserve the user's button visibility. Shared preview support does not prove native drawing support; explain the target host's actual capability. Image toolbars need a separate explicit server capability before upload.
7. If create_candidate_skin refuses the skin, read the reason, fix the manifest or the images, and try again.
8. Once it is installed, offer to switch to it, and do it yourself when the user agrees: read get_preferences, then call update_preferences with that revision and the skin as candidate_skin for light mode (a skin on a light base, light or paper, or on system) and as candidate_skin_dark for dark mode (a dark base, night, ink or shuishan, or system). This also switches the theme to custom, and the input method shows it within a few seconds. Note the global_theme and skins get_preferences reported, so you can switch back if the user does not like it. The user can also pick it in the MSIME settings: open 主题 and choose it under 我的皮肤. If the user wants to share it, offer to publish it with publish_candidate_skin, which uses the account they signed in to in the MSIME settings app; if no one is signed in, ask them to sign in there. Ask whether it should be public, which everyone can see, or private, kept only in their own cloud library. For public, confirm they have the right to share its images and ask which license_assets to use, such as CC-BY-4.0 (reuse with credit) or CC0-1.0 (no conditions). Extended resources require explicit server support."
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

#[cfg(test)]
mod tests {
    #[test]
    fn make_skin_offers_decoration_motion_and_animated_png() {
        let text = super::make_skin_text(None);
        for word in [
            "motion in [candidate_window.decoration]",
            "float, sway, breathe or bounce",
            "motion_period_ms (1200-8000",
            "animated PNG (APNG)",
            "at most 120",
            "show only the first frame",
            "[candidate_window.decoration] (singular",
        ] {
            assert!(text.contains(word), "{word}");
        }
    }

    #[test]
    fn make_skin_keeps_animation_on_any_base_and_canvas_for_full_image_skins() {
        let text = super::make_skin_text(None);
        for word in [
            "Use schema_version = 2 for a lively skin",
            "on any base, so keep base = system or a built-in theme",
            "Use base = \"canvas\" only when",
            "It needs schema_version = 2 (version 1 with canvas is refused)",
            "[candidate_window.border_image]",
            "[[candidate_window.decorations]]",
            "a host without that renderer",
            "candidate windows draw these canvas textures using their platform image codecs",
            "opaque canvas look",
            "macOS, Windows, iOS, Android and HarmonyOS candidate windows draw these canvas textures",
            "(Linux for now)",
            "#RRGGBB or #RRGGBBAA",
            "require explicit server support before upload",
        ] {
            assert!(text.contains(word), "{word}");
        }
    }

    #[test]
    fn make_skin_switches_to_the_skin_it_made() {
        let text = super::make_skin_text(None);
        for word in [
            "call update_preferences with that revision and the skin as candidate_skin",
            "as candidate_skin_dark for dark mode",
            "so you can switch back",
        ] {
            assert!(text.contains(word), "{word}");
        }
    }

    #[test]
    fn make_skin_asks_before_publishing() {
        let text = super::make_skin_text(None);
        for word in [
            "publish it with publish_candidate_skin",
            "ask them to sign in there",
            "Ask whether it should be public, which everyone can see, or private",
            "confirm they have the right to share its images",
            "license_assets",
        ] {
            assert!(text.contains(word), "{word}");
        }
    }

    #[test]
    fn make_skin_passes_images_as_files() {
        let text = super::make_skin_text(None);
        for word in [
            "never write base64 by hand",
            "pass the files' absolute paths in image_files",
            "do not copy base64 into the call",
        ] {
            assert!(text.contains(word), "{word}");
        }
    }
}
