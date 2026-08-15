use std::{fmt::Write, path::PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The registry format this build reads.
///
/// Same bargain as a manifest's schema version: a listing written to a newer
/// shape is refused rather than half understood. It says nothing about the
/// adapters listed, so adding or updating one leaves it alone.
pub const REGISTRY_VERSION: u32 = 1;

pub const DEFAULT_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/pkgforge/aeris-registry/main/registry.toml";

/// What the registry aeris ships knowing about is called.
pub const DEFAULT_REGISTRY_NAME: &str = "pkgforge";

/// A registry to read: where it is, and what to call it.
///
/// Written either as the address on its own, or as a table naming it. The
/// name is what the window shows, since an address is no label.
#[derive(Debug, Clone, serde::Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source {
    Url(String),
    Named { name: String, url: String },
}

impl Source {
    pub fn url(&self) -> &str {
        match self {
            Source::Url(url) => url,
            Source::Named { url, .. } => url,
        }
    }

    /// What to call it. Unnamed, the host it is read from stands in, which is
    /// shorter than the address and enough to tell two apart.
    pub fn name(&self) -> String {
        match self {
            Source::Named { name, .. } => name.clone(),
            Source::Url(url) if url == DEFAULT_REGISTRY_URL => DEFAULT_REGISTRY_NAME.to_string(),
            Source::Url(url) => host_of(url),
        }
    }
}

fn host_of(url: &str) -> String {
    let file = || {
        std::path::Path::new(url)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| url.to_string())
    };

    // A path names no host, and neither does `file://`, so the file it points
    // at stands in for one.
    match url.split_once("://") {
        None | Some(("file", _)) => file(),
        Some((_, rest)) => rest
            .split('/')
            .next()
            .filter(|host| !host.is_empty())
            .map(str::to_string)
            .unwrap_or_else(file),
    }
}

#[derive(Debug, Deserialize)]
pub struct Registry {
    pub registry: RegistryMeta,
    #[serde(default)]
    pub plugins: Vec<PluginEntry>,
}

#[derive(Debug, Deserialize)]
pub struct RegistryMeta {
    pub version: u32,
    pub updated: String,
}

/// One adapter the registry offers, which is a manifest and nothing more.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub manifest_url: String,
    #[serde(default)]
    pub manifest_checksum_sha256: String,
    #[serde(default)]
    pub repo_url: String,
    /// What the registry it was read from is called. Not part of the listing
    /// itself: a registry does not name itself, so it is filled in on the way
    /// past.
    #[serde(skip)]
    pub source: String,
}

/// Where a manifest fetched from the registry is kept, which is the same
/// place a hand-written one goes.
fn adapter_path(id: &str) -> PathBuf {
    crate::adapters::command::manifest::search_paths()
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("./adapters"))
        .join(format!("{id}.toml"))
}

/// Where the last registry that was read is kept.
///
/// This is a copy of something fetchable, so it belongs with the caches:
/// losing it costs one request, not any state.
fn cache_path(source: &str) -> PathBuf {
    let mut named = String::new();
    for byte in Sha256::digest(source.as_bytes()).iter().take(8) {
        let _ = write!(named, "{byte:02x}");
    }

    crate::xdg::cache_home()
        .join("aeris")
        .join(format!("registry-{named}.toml"))
}

/// The registry as it was last read, and when that was.
///
/// A listing from yesterday beats an empty page, so long as it is clear it
/// is from yesterday.
pub fn cached_registry(source: &str) -> Option<(Registry, std::time::SystemTime)> {
    let path = cache_path(source);
    let text = std::fs::read_to_string(&path).ok()?;
    let registry: Registry = toml::from_str(&text).ok()?;
    if registry.registry.version > REGISTRY_VERSION {
        return None;
    }

    let read_at = std::fs::metadata(&path).ok()?.modified().ok()?;

    Some((registry, read_at))
}

/// Whether the copy on disk is old enough to be worth replacing.
///
/// No copy at all counts as stale, and so does one whose age cannot be told.
pub fn cache_is_stale(source: &str, within: std::time::Duration) -> bool {
    let Some((_, read_at)) = cached_registry(source) else {
        return true;
    };

    read_at.elapsed().map(|age| age > within).unwrap_or(true)
}

fn write_cache(source: &str, text: &str) {
    let path = cache_path(source);
    let wrote = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(&path, text));

    if let Err(e) = wrote {
        // Worth saying, but not worth failing over: the listing was read.
        log::warn!("could not keep a copy of the registry: {e}");
    }
}

/// Read the registry from an HTTP(S) URL or a local path, falling back to
/// the built-in default when no source is given.
pub fn fetch_registry(url: &str) -> Result<Registry, String> {
    let body = read_text(url)?;
    let registry: Registry =
        toml::from_str(&body).map_err(|e| format!("Failed to parse registry: {e}"))?;

    if registry.registry.version > REGISTRY_VERSION {
        return Err(format!(
            "the registry is written in version {}, and this aeris reads up to {REGISTRY_VERSION}",
            registry.registry.version
        ));
    }

    write_cache(url, &body);

    Ok(registry)
}

/// Read every registry named, as one listing.
///
/// A registry named earlier is trusted first: where two offer the same
/// adapter, the earlier one is what is on offer and the later is left out.
/// Offering both would show it twice, and each would go on offering an update
/// to the other, since an installed adapter is known by its id alone.
///
/// A registry that cannot be read does not take the others down with it; it
/// is named in the errors instead.
pub fn fetch_all(sources: &[Source]) -> (Vec<PluginEntry>, Vec<String>) {
    let mut listings = Vec::new();
    let mut errors = Vec::new();

    for source in sources {
        match fetch_registry(source.url()) {
            Ok(registry) => listings.push(from(&source.name(), registry.plugins)),
            Err(e) => errors.push(format!("{}: {e}", source.name())),
        }
    }

    (merge(listings), errors)
}

/// The registries already read, as one listing.
pub fn cached_all(sources: &[Source]) -> (Vec<PluginEntry>, Option<std::time::SystemTime>) {
    let mut listings = Vec::new();
    let mut oldest = None;

    for source in sources {
        let Some((registry, read_at)) = cached_registry(source.url()) else {
            continue;
        };
        listings.push(from(&source.name(), registry.plugins));
        oldest = Some(oldest.map_or(read_at, |seen: std::time::SystemTime| seen.min(read_at)));
    }

    (merge(listings), oldest)
}

/// Whether any of the registries is old enough to be worth reading again.
pub fn any_stale(sources: &[Source], within: std::time::Duration) -> bool {
    sources
        .iter()
        .any(|source| cache_is_stale(source.url(), within))
}

fn from(source: &str, plugins: Vec<PluginEntry>) -> Vec<PluginEntry> {
    plugins
        .into_iter()
        .map(|mut entry| {
            entry.source = source.to_string();
            entry
        })
        .collect()
}

/// Fold listings into one, keeping the first offer of each adapter.
pub fn merge(listings: Vec<Vec<PluginEntry>>) -> Vec<PluginEntry> {
    let mut seen: Vec<String> = Vec::new();
    let mut offered = Vec::new();

    for listing in listings {
        for entry in listing {
            if seen.iter().any(|id| *id == entry.id) {
                continue;
            }
            seen.push(entry.id.clone());
            offered.push(entry);
        }
    }

    offered
}

/// Fetch an adapter's manifest and put it where aeris looks for one.
///
/// A manifest is read before it is kept, so a broken one is refused here
/// rather than at the next start. The manifest URL may point at the network
/// or at a local file.
pub fn download_plugin(entry: &PluginEntry) -> Result<PathBuf, String> {
    if entry.manifest_url.is_empty() {
        return Err(format!("{} offers no manifest", entry.id));
    }

    let manifest = read_bytes(&entry.manifest_url)?;
    if !entry.manifest_checksum_sha256.is_empty() {
        verify_checksum(&manifest, &entry.manifest_checksum_sha256)?;
    }

    let text = String::from_utf8(manifest)
        .map_err(|_| format!("{} sent a manifest that is not text", entry.id))?;
    let parsed = crate::adapters::command::manifest::parse(&text)
        .map_err(|e| format!("{}: {e}", entry.id))?;

    if parsed.id != entry.id {
        return Err(format!(
            "the registry calls this {} and the manifest calls it {}",
            entry.id, parsed.id
        ));
    }

    let path = adapter_path(&entry.id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create adapter dir: {e}"))?;
    }
    std::fs::write(&path, &text).map_err(|e| format!("Failed to write manifest: {e}"))?;

    Ok(path)
}

pub fn remove_plugin(id: &str) -> Result<(), String> {
    let path = adapter_path(id);
    if !path.exists() {
        return Ok(());
    }

    std::fs::remove_file(&path).map_err(|e| format!("Failed to remove {}: {e}", path.display()))
}

/// The newer version an installed adapter could be updated to, if any.
///
/// Versions are compared the way a manager's own are, since a registry says
/// whatever the manager says about itself.
pub fn update_for(entry: &PluginEntry) -> Option<String> {
    let installed = installed_plugin_version(&entry.id)?;

    (!entry.version.is_empty()
        && entry.version != installed
        && crate::adapters::command::version::at_least(&entry.version, &installed))
    .then(|| entry.version.clone())
}

/// The command an adapter already on disk needs in order to work.
///
/// A manifest is kept whether or not that command is there, so this is how
/// the page can say what is still missing.
pub fn installed_needs(id: &str) -> Option<String> {
    let text = std::fs::read_to_string(adapter_path(id)).ok()?;
    let manifest = crate::adapters::command::manifest::parse(&text).ok()?;

    Some(manifest.detect.command)
}

pub fn installed_plugin_version(id: &str) -> Option<String> {
    let text = std::fs::read_to_string(adapter_path(id)).ok()?;
    let manifest = crate::adapters::command::manifest::parse(&text).ok()?;

    (!manifest.version.is_empty()).then_some(manifest.version)
}

/// Whether a source is fetched over HTTP rather than read from disk.
fn is_remote(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

/// Turn a source into a path, dropping a `file://` prefix and expanding `~`.
fn local_path(source: &str) -> PathBuf {
    let stripped = source.strip_prefix("file://").unwrap_or(source);
    shellexpand::tilde(stripped).to_string().into()
}

/// Read bytes from an HTTP(S) URL or a local file.
fn read_bytes(source: &str) -> Result<Vec<u8>, String> {
    if is_remote(source) {
        let resp = ureq::get(source)
            .call()
            .map_err(|e| format!("Download failed: {e}"))?;

        return resp
            .into_body()
            .read_to_vec()
            .map_err(|e| format!("Failed to read download body: {e}"));
    }

    let path = local_path(source);
    std::fs::read(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))
}

/// Read text from an HTTP(S) URL or a local file.
fn read_text(source: &str) -> Result<String, String> {
    let bytes = read_bytes(source)?;
    String::from_utf8(bytes).map_err(|e| format!("{source} is not valid UTF-8: {e}"))
}

fn verify_checksum(data: &[u8], expected_hex: &str) -> Result<(), String> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut actual_hex = String::with_capacity(64);
    for byte in result {
        write!(&mut actual_hex, "{byte:02x}").unwrap();
    }
    if actual_hex != expected_hex {
        return Err(format!(
            "Checksum mismatch: expected {expected_hex}, got {actual_hex}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(id: &str, version: &str) -> PluginEntry {
        PluginEntry {
            id: id.to_string(),
            name: id.to_string(),
            version: version.to_string(),
            description: String::new(),
            manifest_url: String::new(),
            manifest_checksum_sha256: String::new(),
            repo_url: String::new(),
            source: String::new(),
        }
    }

    fn from_source(name: &str, ids: &[(&str, &str)]) -> Vec<PluginEntry> {
        from(
            name,
            ids.iter()
                .map(|(id, version)| offered(id, version))
                .collect(),
        )
    }

    #[test]
    fn the_registry_named_first_is_the_one_offering_an_adapter() {
        // Both carry `am`, and the second carries a higher version. Offering
        // both would show it twice, and each would go on offering an update
        // to the other, since an installed adapter is known by its id alone.
        let merged = merge(vec![
            from_source("work", &[("am", "1.0.0"), ("internal", "2.0.0")]),
            from_source("pkgforge", &[("am", "9.9.9"), ("pacstall", "6.4.5")]),
        ]);

        let named: Vec<(&str, &str, &str)> = merged
            .iter()
            .map(|e| (e.id.as_str(), e.version.as_str(), e.source.as_str()))
            .collect();

        assert_eq!(
            named,
            [
                ("am", "1.0.0", "work"),
                ("internal", "2.0.0", "work"),
                ("pacstall", "6.4.5", "pkgforge"),
            ]
        );
    }

    #[test]
    fn a_registry_is_named_after_itself_when_it_is_not_named() {
        assert_eq!(
            Source::Named {
                name: "work".into(),
                url: "https://example.invalid/r.toml".into(),
            }
            .name(),
            "work"
        );
        assert_eq!(
            Source::Url(DEFAULT_REGISTRY_URL.to_string()).name(),
            DEFAULT_REGISTRY_NAME
        );
        assert_eq!(
            Source::Url("https://adapters.example.invalid/r.toml".into()).name(),
            "adapters.example.invalid"
        );
        assert_eq!(
            Source::Url("/srv/aeris/internal.toml".into()).name(),
            "internal"
        );
        assert_eq!(Source::Url("file:///srv/r.toml".into()).name(), "r");
    }

    #[test]
    fn a_registry_is_written_as_an_address_or_as_a_table() {
        #[derive(serde::Deserialize)]
        struct Config {
            registries: Vec<Source>,
        }

        let config: Config = toml::from_str(
            r#"
            registries = [
              "https://adapters.example.invalid/r.toml",
              { name = "work", url = "file:///srv/r.toml" },
            ]
            "#,
        )
        .expect("both forms should read");

        assert_eq!(config.registries[0].name(), "adapters.example.invalid");
        assert_eq!(config.registries[1].name(), "work");
        assert_eq!(config.registries[1].url(), "file:///srv/r.toml");
    }

    fn install(id: &str, body: &str) {
        let path = adapter_path(id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn manifest_saying(id: &str, version: &str) -> String {
        format!(
            r#"schema_version = 1
id = "{id}"
name = "Test"
version = "{version}"

[detect]
command = "true"
"#
        )
    }

    #[test]
    fn an_adapter_nobody_has_is_not_an_update() {
        assert_eq!(update_for(&offered("absent-adapter-test", "2.0")), None);
    }

    #[test]
    fn a_newer_version_in_the_registry_is_offered() {
        let id = "update-check-test";
        install(id, &manifest_saying(id, "1.0"));

        assert_eq!(update_for(&offered(id, "1.1")).as_deref(), Some("1.1"));
        // The same version, and an older one, are both nothing to do.
        assert_eq!(update_for(&offered(id, "1.0")), None);
        assert_eq!(update_for(&offered(id, "0.9")), None);

        let _ = std::fs::remove_file(adapter_path(id));
    }

    #[test]
    fn http_is_remote_and_a_path_is_not() {
        assert!(is_remote("https://example.com/registry.toml"));
        assert!(is_remote("http://example.com/registry.toml"));
        assert!(!is_remote("/etc/aeris/registry.toml"));
        assert!(!is_remote("./registry.toml"));
        assert!(!is_remote("file:///etc/aeris/registry.toml"));
    }

    #[test]
    fn a_local_registry_is_read_from_disk() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("aeris-registry-{nanos}.toml"));
        std::fs::write(&path, "[registry]\nversion = 1\nupdated = \"now\"\n").unwrap();

        let registry = fetch_registry(path.to_str().unwrap()).expect("should read the registry");
        assert_eq!(registry.registry.version, 1);
        assert!(registry.plugins.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_local_registry_explains_itself() {
        let err = fetch_registry("/no/such/aeris-registry.toml")
            .expect_err("should not read a missing file");
        assert!(err.contains("Failed to read"), "{err}");
    }
}
