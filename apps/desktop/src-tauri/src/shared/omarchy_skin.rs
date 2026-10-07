//! The candidate skin that follows the Omarchy theme.
//!
//! Omarchy (basecamp/omarchy) describes each theme with a `colors.toml` and runs `~/.config/omarchy/hooks/theme-set.d/*` after it switches theme. The hook MSIME installs there runs `msime-linux-settings --sync-omarchy-theme`, which reads the new palette through Omarchy's own resolver (`omarchy-theme-color --all`, the one its templates use, so legacy `colorN` themes resolve the same way), writes it as the `omarchy` package in the skin root and publishes the catalog the Linux hosts read. The package is an ordinary skin: it is listed, chosen and removed like any other, and it only recolours the candidate window once the user picks it.
use std::collections::HashMap;
use std::io;
use std::path::Path;

/// The package folder and manifest id.
pub(crate) const SKIN_ID: &str = "omarchy";
const MAX_MANIFEST_BYTES: usize = 64 * 1024;

/// The palette `omarchy-theme-color --all` prints: one `key<TAB>value` line per resolved key.
pub(crate) fn parse_resolved_colors(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .filter(|(key, value)| !key.is_empty() && !value.is_empty())
        .collect()
}

/// The `skin.toml` for a resolved palette, or none when it lacks the background or foreground every theme defines. A colour the shared layer cannot read is left out, so that slot falls back to the base theme rather than the manifest being refused.
///
/// Omarchy draws its menus and notifications square, on the theme background with an accent border, and the card follows it: `selection` fills the chosen candidate, `lighter_background` the one under the pointer, and the numbers and translations take `dark_foreground`. The same palette serves both modes, because Omarchy already moves the desktop's colour scheme with the theme (`omarchy-theme-set-gnome`), so the mode the host draws in is the theme's own.
pub(crate) fn skin_manifest(colors: &HashMap<String, String>) -> Option<String> {
    let color = |key: &str| {
        colors
            .get(key)
            .and_then(|value| msime_client_core::skin::theme::normalized_color(value))
    };
    color("background")?;
    color("foreground")?;
    let slots = [
        ("surface", "background"),
        ("border", "accent"),
        ("text", "foreground"),
        ("number", "dark_foreground"),
        ("translation", "dark_foreground"),
        ("accent", "accent"),
        ("selected", "selection"),
        ("hover", "lighter_background"),
    ];
    let palette: String = slots
        .iter()
        .filter_map(|(slot, key)| color(key).map(|value| format!("{slot} = \"{value}\"\n")))
        .collect();
    Some(format!(
        "schema_version = 1\n\
         id = \"{SKIN_ID}\"\n\
         name = \"Omarchy\"\n\
         version = \"1\"\n\
         base = \"system\"\n\
         description = \"跟随当前 Omarchy 主题的配色，切换 Omarchy 主题时自动更新。\"\n\
         \n\
         [supports]\n\
         layouts = [\"horizontal\", \"vertical\"]\n\
         themes = [\"dark\", \"light\"]\n\
         \n\
         [candidate_window]\n\
         corner_radius_dip = 0\n\
         \n\
         [candidate.dark]\n\
         {palette}\
         \n\
         [candidate.light]\n\
         {palette}"
    ))
}

/// Write the package into `root`, leaving an identical manifest untouched. Returns whether the file changed.
pub(crate) fn install(root: &Path, manifest: &str) -> io::Result<bool> {
    let path = root.join(SKIN_ID).join("skin.toml");
    if let Ok(file) = super::atomic_file::open_private(&path) {
        let current = super::bounded_body::read_bounded(file, MAX_MANIFEST_BYTES)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "skin manifest too large"))?;
        if current == manifest.as_bytes() {
            return Ok(false);
        }
    }
    super::atomic_file::write(&path, manifest.as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    // tokyo-night's colors.toml as omarchy-theme-color --all resolves it, trimmed to the keys the skin reads plus one it ignores.
    const TOKYO_NIGHT: &str = "accent\t#7aa2f7\nbackground\t#1a1b26\ndark_foreground\t#565f89\nforeground\t#a9b1d6\nlighter_background\t#24283b\nmode\tdark\nselection\t#292e42\n";

    #[test]
    fn the_package_scans_with_the_theme_palette_in_both_modes() {
        let manifest = skin_manifest(&parse_resolved_colors(TOKYO_NIGHT)).unwrap();
        let root = tempfile::tempdir().unwrap();
        assert!(install(root.path(), &manifest).unwrap());
        let catalog = msime_client_core::skin::catalog::scan(root.path());
        assert!(catalog.issues.is_empty(), "{:?}", catalog.issues);
        let package = &catalog.packages[0];
        assert_eq!(package.id, SKIN_ID);
        assert_eq!(package.corner_radius_dip, Some(0.0));
        for palette in [&package.candidate.dark, &package.candidate.light] {
            assert_eq!(palette.surface.as_deref(), Some("#1A1B26"));
            assert_eq!(palette.border.as_deref(), Some("#7AA2F7"));
            assert_eq!(palette.selected.as_deref(), Some("#292E42"));
            assert_eq!(palette.hover.as_deref(), Some("#24283B"));
            assert_eq!(palette.number.as_deref(), Some("#565F89"));
        }
    }

    #[test]
    fn an_unreadable_colour_falls_back_instead_of_breaking_the_package() {
        let colors = parse_resolved_colors(
            "background\t#ffffff\nforeground\t#000000\nselection\tnot-a-colour\n",
        );
        let manifest = skin_manifest(&colors).unwrap();
        assert!(!manifest.contains("selected"));
        let root = tempfile::tempdir().unwrap();
        install(root.path(), &manifest).unwrap();
        assert_eq!(
            msime_client_core::skin::catalog::scan(root.path())
                .packages
                .len(),
            1
        );
    }

    #[test]
    fn a_palette_without_background_or_foreground_writes_nothing() {
        assert!(skin_manifest(&parse_resolved_colors("background\t#1a1b26\n")).is_none());
        assert!(skin_manifest(&parse_resolved_colors("")).is_none());
    }

    #[test]
    fn reinstalling_the_same_palette_leaves_the_file_alone() {
        let manifest = skin_manifest(&parse_resolved_colors(TOKYO_NIGHT)).unwrap();
        let root = tempfile::tempdir().unwrap();
        assert!(install(root.path(), &manifest).unwrap());
        assert!(!install(root.path(), &manifest).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn install_does_not_follow_a_linked_existing_manifest() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let manifest = skin_manifest(&parse_resolved_colors(TOKYO_NIGHT)).unwrap();
        let external = outside.path().join("skin.toml");
        std::fs::write(&external, manifest.as_bytes()).unwrap();
        std::fs::create_dir(root.path().join(SKIN_ID)).unwrap();
        symlink(&external, root.path().join(SKIN_ID).join("skin.toml")).unwrap();

        assert!(install(root.path(), &manifest).unwrap());
        assert_eq!(std::fs::read(&external).unwrap(), manifest.as_bytes());
    }
}
