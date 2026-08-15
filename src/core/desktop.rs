//! What the desktop entries on disk say about installed applications.
//!
//! A manager that integrates with the desktop writes a `.desktop` file and an
//! icon for what it installs, which is metadata aeris can show without asking
//! anyone for it. Where a distribution ships an AppStream catalog there is more
//! to be had, but this much is already on every machine.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// Icon sizes are chosen against this, since a card shows an icon small and
/// decoding a 512 pixel image to draw it at 28 is waste.
const PREFERRED_ICON_SIZE: u32 = 64;

const ICON_EXTENSIONS: [&str; 3] = ["svg", "png", "xpm"];

/// One application, as its desktop entry describes it.
#[derive(Debug, Clone)]
pub struct DesktopEntry {
    /// The one line summary the entry carries, which is what a launcher shows
    /// under the name.
    pub comment: Option<String>,
    /// Where the icon was found, if it was found at all. Resolved when the
    /// entry is read rather than when it is drawn, so nothing searches the
    /// icon theme while a list is scrolling.
    pub icon: Option<PathBuf>,
}

/// Every desktop entry found, indexed by what a package might be called.
#[derive(Debug, Default)]
pub struct Desktop {
    entries: Vec<DesktopEntry>,
    by_binary: HashMap<String, usize>,
    by_stem: HashMap<String, usize>,
}

impl Desktop {
    /// Read every entry the desktop specification says to look at.
    ///
    /// Entries found earlier win, so what a person installed for themselves
    /// describes the package rather than what a distribution shipped.
    pub fn load() -> Self {
        let mut desktop = Self::default();

        for dir in application_dirs() {
            let Ok(listing) = std::fs::read_dir(&dir) else {
                continue;
            };

            for found in listing.flatten() {
                let path = found.path();
                if path.extension().is_none_or(|ext| ext != "desktop") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Some(parsed) = parse(&text) else {
                    continue;
                };

                let at = desktop.entries.len();
                if let Some(binary) = parsed.binary {
                    desktop.by_binary.entry(binary).or_insert(at);
                }
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    desktop.by_stem.entry(stem.to_string()).or_insert(at);
                }
                desktop.entries.push(DesktopEntry {
                    comment: parsed.comment,
                    icon: parsed.icon.as_deref().and_then(icon_path),
                });
            }
        }

        desktop
    }

    /// What a package is called on disk, matched to what it installed.
    ///
    /// The command it installed is the surer match: a manager decorates the
    /// file name it writes, but the binary keeps the name the package has.
    pub fn find(&self, package: &str) -> Option<&DesktopEntry> {
        self.by_binary
            .get(package)
            .or_else(|| self.by_stem.get(package))
            .and_then(|at| self.entries.get(*at))
    }
}

/// The `[Desktop Entry]` group, which is the only one worth reading.
///
/// A file may carry further groups describing actions, and those repeat `Name`
/// and `Exec` for something else entirely.
struct Parsed {
    comment: Option<String>,
    icon: Option<String>,
    binary: Option<String>,
}

fn parse(text: &str) -> Option<Parsed> {
    let mut name = None;
    let mut comment = None;
    let mut icon = None;
    let mut exec = None;
    let mut try_exec = None;
    let mut inside = false;
    let mut hidden = false;

    for line in text.lines() {
        let line = line.trim();

        if line.starts_with('[') {
            if inside {
                break;
            }
            inside = line == "[Desktop Entry]";
            continue;
        }
        if !inside {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();

        match key.trim() {
            "Name" => name = Some(value.to_string()),
            "Comment" => comment = Some(value.to_string()),
            "Icon" => icon = Some(value.to_string()),
            "Exec" => exec = Some(value.to_string()),
            "TryExec" => try_exec = Some(value.to_string()),
            "NoDisplay" | "Hidden" => hidden |= value == "true",
            _ => {}
        }
    }

    // A entry without a name is not an entry the specification recognises.
    if hidden || name.is_none() {
        return None;
    }

    Some(Parsed {
        comment,
        icon,
        binary: try_exec.or(exec).as_deref().and_then(binary_of),
    })
}

/// The command a desktop entry runs, without its path or its arguments.
///
/// `Exec` carries field codes such as `%u` and may quote the program, neither
/// of which is part of the name the package goes by.
fn binary_of(exec: &str) -> Option<String> {
    let exec = exec.trim();
    let program = match exec.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next()?,
        None => exec.split_whitespace().next()?,
    };

    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .filter(|name| !name.is_empty())
}

/// Where a named icon lives, following the icon theme specification closely
/// enough for the icons a package manager installs.
pub fn icon_path(icon: &str) -> Option<PathBuf> {
    if icon.starts_with('/') {
        return exact_or_extended(Path::new(icon));
    }

    let mut best: Option<(u32, PathBuf)> = None;

    for root in icon_dirs() {
        // A theme directory holds its sizes; a directory of loose icons, such
        // as pixmaps, holds them directly.
        if let Some(found) = in_flat_dir(&root, icon) {
            best = better(best, (PREFERRED_ICON_SIZE, found));
        }

        let Ok(themes) = std::fs::read_dir(&root) else {
            continue;
        };
        for theme in themes.flatten() {
            let Ok(sizes) = std::fs::read_dir(theme.path()) else {
                continue;
            };
            for size in sizes.flatten() {
                let named = size.file_name();
                let named = named.to_string_lossy();
                let Some(found) = in_flat_dir(&size.path().join("apps"), icon) else {
                    continue;
                };
                best = better(best, (size_of(&named), found));
            }
        }
    }

    best.map(|(_, path)| path)
}

/// Whichever is closer to the size an icon is drawn at, preferring the larger
/// of two that are equally far off so nothing is drawn blurred.
fn better(held: Option<(u32, PathBuf)>, found: (u32, PathBuf)) -> Option<(u32, PathBuf)> {
    let Some(held) = held else {
        return Some(found);
    };

    let distance = |size: u32| size.abs_diff(PREFERRED_ICON_SIZE);
    match distance(found.0).cmp(&distance(held.0)) {
        std::cmp::Ordering::Less => Some(found),
        std::cmp::Ordering::Greater => Some(held),
        std::cmp::Ordering::Equal if found.0 > held.0 => Some(found),
        std::cmp::Ordering::Equal => Some(held),
    }
}

/// The pixel size a theme directory holds, from names such as `128x128`.
/// A scalable directory answers for any size, so it is treated as ideal.
fn size_of(directory: &str) -> u32 {
    if directory == "scalable" || directory == "symbolic" {
        return PREFERRED_ICON_SIZE;
    }

    directory
        .split(['x', '@'])
        .next()
        .and_then(|count| count.parse().ok())
        .unwrap_or(0)
}

fn in_flat_dir(dir: &Path, icon: &str) -> Option<PathBuf> {
    ICON_EXTENSIONS.iter().find_map(|ext| {
        let candidate = dir.join(format!("{icon}.{ext}"));
        candidate.is_file().then_some(candidate)
    })
}

/// An absolute icon, which the specification says carries its extension.
/// Some managers write one without, so the extensions are tried too.
fn exact_or_extended(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }

    let name = path.file_name()?.to_str()?;
    in_flat_dir(path.parent()?, name)
}

fn application_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![crate::xdg::data_home().join("applications")];
    dirs.extend(
        crate::xdg::data_dirs()
            .into_iter()
            .map(|dir| dir.join("applications")),
    );
    dirs
}

fn icon_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![crate::xdg::data_home().join("icons")];

    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".icons"));
    }

    dirs.extend(
        crate::xdg::data_dirs()
            .into_iter()
            .flat_map(|dir| [dir.join("icons"), dir.join("pixmaps")]),
    );
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_group_is_read() {
        let parsed = parse(
            "[Desktop Entry]\n\
             Name=FireDragon\n\
             Comment=Browse the World Wide Web\n\
             Icon=firedragon\n\
             Exec=/home/me/.local/bin/firedragon %u\n\
             \n\
             [Desktop Action new-window]\n\
             Name=New Window\n\
             Comment=Open a window\n\
             Icon=window-new\n\
             Exec=/home/me/.local/bin/firedragon --new-window %u\n",
        )
        .expect("the entry should be read");

        assert_eq!(parsed.comment.as_deref(), Some("Browse the World Wide Web"));
        assert_eq!(parsed.icon.as_deref(), Some("firedragon"));
    }

    #[test]
    fn a_command_is_named_without_its_path_or_arguments() {
        assert_eq!(
            binary_of("/home/me/.local/share/soar/bin/newpipe ").as_deref(),
            Some("newpipe")
        );
        assert_eq!(
            binary_of("\"/usr/bin/my app\" %U").as_deref(),
            Some("my app")
        );
        assert_eq!(binary_of("").as_deref(), None);
    }

    #[test]
    fn an_entry_that_asks_not_to_be_shown_is_skipped() {
        assert!(parse("[Desktop Entry]\nName=Hidden\nNoDisplay=true\n").is_none());
    }

    #[test]
    fn the_size_a_theme_directory_holds_is_read_from_its_name() {
        assert_eq!(size_of("128x128"), 128);
        assert_eq!(size_of("67x80"), 67);
        assert_eq!(size_of("64x64@2"), 64);
        assert_eq!(size_of("scalable"), PREFERRED_ICON_SIZE);
        assert_eq!(size_of("apps"), 0);
    }

    #[test]
    fn the_size_nearest_the_one_drawn_wins() {
        let small = better(None, (32, PathBuf::from("small")));
        let near = better(small, (64, PathBuf::from("near")));
        let large = better(near, (512, PathBuf::from("large")));

        assert_eq!(large.map(|(_, path)| path), Some(PathBuf::from("near")));
    }
}
