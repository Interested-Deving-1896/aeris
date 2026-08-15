//! Icons for packages that are not installed.
//!
//! Nothing on disk describes a package until it is installed, so what a card
//! shows before that comes from a published set of icons, each named for the
//! package it belongs to.
//!
//! An index says which packages have one and where they are served from. Each
//! is fetched the first time it is drawn and kept, so what crosses the wire is
//! what somebody actually looked at rather than a catalogue of everything in
//! case they do.
//!
//! The map alongside it is only for the exceptions: two managers can offer
//! `firefox-bin` and mean different builds of it, and one package can want the
//! icon another package's name is on.

use std::{collections::HashMap, io::Write, path::PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Where the exceptions are read from when nothing says otherwise.
pub const DEFAULT_ICON_MAP_URL: &str =
    "https://raw.githubusercontent.com/pkgforge/aeris-metadata/main/icons.toml";

/// Where the index is read from when nothing says otherwise. It says where the
/// icons themselves live, so moving them does not need a new aeris.
pub const DEFAULT_ICON_INDEX_URL: &str =
    "https://raw.githubusercontent.com/pkgforge/aeris-metadata/main/index.toml";

/// The formats the published set may carry, in the order they are preferred.
/// A drawing that scales is worth more than one that does not.
const ICON_EXTENSIONS: [&str; 2] = ["svg", "png"];

/// Suffixes that say how a package was built rather than what it is, so a
/// package carrying one is drawn with the icon of the program it packages.
///
/// Only ever a fallback: a package with an icon of its own keeps it. `-git`
/// and `-stable` belong here because building from a repository, or naming the
/// channel a project already draws its icon for, changes nothing about what a
/// program is. What is absent is anything branded apart, such as `-nightly` or
/// `-beta`, where the same program really is drawn differently.
const PACKAGING_SUFFIXES: [&str; 7] = [
    "-bin",
    "-deb",
    "-app",
    "-appimage",
    "-static",
    "-git",
    "-stable",
];

/// What a package's icon might be called, the name it goes by first.
///
/// Suffixes come off one after another, since a package can carry more than
/// one of them: `zed-editor-stable-bin` is `zed-editor` built a particular way
/// from a particular channel.
fn named(icon: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(icon), |name| {
        PACKAGING_SUFFIXES
            .iter()
            .find_map(|suffix| name.strip_suffix(suffix))
            .filter(|shorter| !shorter.is_empty())
    })
}

/// How long the index and the exceptions stay good for. A name means the same
/// icon for far longer than a package keeps a version.
pub const REFRESH_AFTER: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Which icon a package is drawn with, where that is not simply its own.
///
/// Empty is the ordinary case: a package called `krita` is drawn with the icon
/// called `krita`, and nothing has to say so.
#[derive(Debug, Default, Deserialize)]
pub struct IconMap {
    /// What any manager offering that name should be drawn with.
    #[serde(default)]
    default: HashMap<String, String>,
    /// Where one manager wants something else, by adapter id.
    #[serde(flatten, default)]
    per_adapter: HashMap<String, HashMap<String, String>>,
}

impl IconMap {
    /// The exceptions as they were last read, which is none until they have
    /// been.
    pub fn cached() -> Self {
        read_cached(map_path())
    }

    /// The icon a package is drawn with, asking the manager's own table before
    /// the one that speaks for every manager.
    ///
    /// A package nobody has said anything about is drawn with its own name,
    /// which is what the published set is keyed by.
    ///
    /// Said of `firefox-developer-edition`, it is said of the `-bin` and `-deb`
    /// of it too: a suffix saying how something was built does not make it a
    /// different program, and repeating the line for each would.
    pub fn icon_of<'a>(&'a self, adapter_id: &str, package: &'a str) -> &'a str {
        named(package)
            .find_map(|name| {
                self.per_adapter
                    .get(adapter_id)
                    .and_then(|said| said.get(name))
                    .or_else(|| self.default.get(name))
            })
            .map(String::as_str)
            .unwrap_or(package)
    }
}

/// Which packages have an icon, and where those icons are served from.
#[derive(Debug, Default, Deserialize)]
pub struct IconIndex {
    /// Where the icons are, without a trailing slash.
    #[serde(default)]
    base: String,
    /// Package name to the extension its icon is published as.
    #[serde(default)]
    icons: HashMap<String, String>,
}

impl IconIndex {
    /// The index as it was last read, which is empty until it has been.
    pub fn cached() -> Self {
        read_cached(index_path())
    }

    /// Where a package's icon is published, if one is.
    ///
    /// Answering nothing is the ordinary case: most packages are command line
    /// tools, and aeris draws those as a package.
    pub fn url_of(&self, icon: &str) -> Option<String> {
        let (name, extension) = self.published(icon)?;
        (!self.base.is_empty()).then(|| format!("{}/{name}.{extension}", self.base))
    }

    /// What a package's icon is called on disk once it has been fetched.
    pub fn file_name(&self, icon: &str) -> Option<String> {
        let (name, extension) = self.published(icon)?;
        Some(format!("{name}.{extension}"))
    }

    /// The name the set carries this package's icon under, and as what.
    fn published<'a>(&self, icon: &'a str) -> Option<(&'a str, &str)> {
        named(icon).find_map(|name| Some((name, self.icons.get(name)?.as_str())))
    }

    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }
}

/// Read the exceptions again and keep them.
pub fn refresh_map(source: &str) -> Result<IconMap, String> {
    refresh_into(source, map_path())
}

/// Read the index again and keep it.
pub fn refresh_index(source: &str) -> Result<IconIndex, String> {
    refresh_into(source, index_path())
}

fn refresh_into<T: Default + for<'de> Deserialize<'de>>(
    source: &str,
    keep_at: PathBuf,
) -> Result<T, String> {
    let text = read(source)?;
    let parsed: T = toml::from_str(&text).map_err(|e| format!("{source}: {e}"))?;

    if let Some(parent) = keep_at.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(keep_at, &text);

    Ok(parsed)
}

fn read_cached<T: Default + for<'de> Deserialize<'de>>(from: PathBuf) -> T {
    std::fs::read_to_string(from)
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

fn read(source: &str) -> Result<String, String> {
    if is_fetchable(source) {
        return ureq::get(source)
            .call()
            .map_err(|e| format!("{source}: {e}"))?
            .into_body()
            .read_to_string()
            .map_err(|e| format!("{source}: {e}"));
    }

    let path = shellexpand::tilde(source.strip_prefix("file://").unwrap_or(source)).to_string();
    std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))
}

/// Where a package's icon already on disk is kept, if it is.
///
/// The set may carry either format, so both are looked for and the one that
/// scales wins where a package has been given both.
pub fn cached_icon(icon: &str) -> Option<PathBuf> {
    if is_fetchable(icon) {
        let path = icon_dir().join(digest_of(icon));
        return path.is_file().then_some(path);
    }

    named(icon).find_map(|name| {
        ICON_EXTENSIONS.iter().find_map(|extension| {
            let path = icon_dir().join(format!("{name}.{extension}"));
            path.is_file().then_some(path)
        })
    })
}

/// Fetch one package's icon and keep it.
///
/// Written under its final name only once it is whole, so a fetch cut short
/// cannot leave something behind that is read as an icon ever after.
pub fn fetch_icon(index: &IconIndex, icon: &str) -> Result<PathBuf, String> {
    let (url, name) = match (index.url_of(icon), index.file_name(icon)) {
        (Some(url), Some(name)) => (url, name),
        // An icon named outright rather than published: fetched as given, and
        // kept under the digest of where it came from.
        _ if is_fetchable(icon) => (icon.to_string(), digest_of(icon)),
        _ => return Err(format!("nothing publishes an icon for {icon}")),
    };

    let path = icon_dir().join(name);
    if path.is_file() {
        return Ok(path);
    }

    let bytes = ureq::get(&url)
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .into_body()
        .read_to_vec()
        .map_err(|e| format!("{url}: {e}"))?;

    if bytes.is_empty() {
        return Err(format!("{url}: nothing was sent"));
    }

    std::fs::create_dir_all(icon_dir()).map_err(|e| e.to_string())?;

    let partial = path.with_extension("part");
    let mut file = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(&partial, &path).map_err(|e| e.to_string())?;

    Ok(path)
}

/// Whether something names where to get it rather than naming a package.
pub fn is_fetchable(source: &str) -> bool {
    source.starts_with("https://") || source.starts_with("http://")
}

/// A URL is not a file name, so one is kept under the digest of the other.
fn digest_of(url: &str) -> String {
    let mut digest = String::new();
    for byte in Sha256::digest(url.as_bytes()).iter().take(16) {
        digest.push_str(&format!("{byte:02x}"));
    }
    digest
}

fn icon_dir() -> PathBuf {
    cache_dir().join("icons")
}

fn map_path() -> PathBuf {
    cache_dir().join("icons.toml")
}

fn index_path() -> PathBuf {
    cache_dir().join("index.toml")
}

fn cache_dir() -> PathBuf {
    crate::xdg::cache_home().join("aeris")
}

/// Whether what is on disk is old enough to be worth reading again.
pub fn is_stale(within: std::time::Duration) -> bool {
    let Ok(written) = std::fs::metadata(index_path()).and_then(|m| m.modified()) else {
        return true;
    };

    written
        .elapsed()
        .map(|since| since > within)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exceptions() -> IconMap {
        toml::from_str(
            r#"
            [default]
            krita-devel = "krita"

            [am]
            firefox-bin = "firefox-nightly"
            "#,
        )
        .expect("the sample should read")
    }

    fn index() -> IconIndex {
        toml::from_str(
            r#"
            base = "https://example.invalid/icons"

            [icons]
            krita = "png"
            soar = "svg"
            "#,
        )
        .expect("the sample should read")
    }

    #[test]
    fn a_package_nobody_has_spoken_for_is_drawn_with_its_own_name() {
        assert_eq!(
            exceptions().icon_of("soar", "krita"),
            "krita",
            "the ordinary case needs no entry at all"
        );
    }

    #[test]
    fn one_package_can_be_drawn_with_another_name() {
        assert_eq!(
            exceptions().icon_of("soar", "krita-devel"),
            "krita",
            "so the set carries one icon rather than a copy per package"
        );
    }

    #[test]
    fn saying_it_of_a_name_says_it_of_how_that_was_packaged() {
        let map: IconMap = toml::from_str(
            r#"
            [default]
            firefox-developer-edition = "firefox-devedition"
            "#,
        )
        .expect("the sample should read");

        for package in [
            "firefox-developer-edition",
            "firefox-developer-edition-bin",
            "firefox-developer-edition-deb",
        ] {
            assert_eq!(
                map.icon_of("am", package),
                "firefox-devedition",
                "{package} is the same program, built another way"
            );
        }
    }

    #[test]
    fn a_manager_that_means_something_else_by_a_name_wins() {
        let map = exceptions();

        assert_eq!(map.icon_of("am", "firefox-bin"), "firefox-nightly");
        assert_eq!(
            map.icon_of("soar", "firefox-bin"),
            "firefox-bin",
            "a manager with no say of its own is drawn with its own name"
        );
    }

    #[test]
    fn an_icon_is_asked_for_where_the_index_says_it_is() {
        let index = index();

        assert_eq!(
            index.url_of("soar").as_deref(),
            Some("https://example.invalid/icons/soar.svg"),
            "the extension comes from the index rather than from a guess"
        );
        assert_eq!(index.file_name("krita").as_deref(), Some("krita.png"));
    }

    #[test]
    fn a_suffix_saying_how_it_was_packaged_is_not_part_of_the_name() {
        let index = index();

        assert_eq!(
            index.url_of("krita-bin").as_deref(),
            Some("https://example.invalid/icons/krita.png"),
            "the same program, built another way"
        );
        assert_eq!(index.file_name("krita-deb").as_deref(), Some("krita.png"));
        assert_eq!(
            index.url_of("krita-git").as_deref(),
            Some("https://example.invalid/icons/krita.png"),
            "built from a repository, still the same program"
        );
        assert_eq!(
            index.url_of("krita-nightly"),
            None,
            "a channel a project brands apart is left alone"
        );
    }

    #[test]
    fn more_than_one_suffix_comes_off() {
        let index: IconIndex = toml::from_str(
            r#"
            base = "https://example.invalid/icons"

            [icons]
            zed-editor = "svg"
            "#,
        )
        .expect("the sample should read");

        assert_eq!(
            index.file_name("zed-editor-stable-bin").as_deref(),
            Some("zed-editor.svg"),
            "a channel and a way of building are both only how it was packaged"
        );
    }

    #[test]
    fn a_package_with_an_icon_of_its_own_keeps_it() {
        let index: IconIndex = toml::from_str(
            r#"
            base = "https://example.invalid/icons"

            [icons]
            soar = "svg"
            soar-bin = "png"
            "#,
        )
        .expect("the sample should read");

        assert_eq!(
            index.file_name("soar-bin").as_deref(),
            Some("soar-bin.png"),
            "trimming is a fallback, never an override"
        );
    }

    #[test]
    fn a_package_the_index_does_not_name_is_never_asked_for() {
        let index = index();

        assert_eq!(index.url_of("ripgrep"), None);
        assert!(
            fetch_icon(&index, "ripgrep").is_err(),
            "asking anyway is an error rather than a request that will 404"
        );
    }

    #[test]
    fn an_icon_named_outright_is_kept_under_the_digest_of_where_it_came_from() {
        let one = digest_of("https://example.invalid/a.png");
        let other = digest_of("https://example.invalid/b.png");

        assert_eq!(one, digest_of("https://example.invalid/a.png"));
        assert_ne!(one, other);
        assert!(!one.contains('/'), "a digest has to be a usable file name");
    }
}
