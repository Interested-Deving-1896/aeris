//! Where the desktop specification says a program's files belong.

use std::path::PathBuf;

/// Where per-user configuration goes, honouring `XDG_CONFIG_HOME`.
pub fn config_home() -> PathBuf {
    home_relative("XDG_CONFIG_HOME", ".config")
}

/// Where per-user data goes, honouring `XDG_DATA_HOME`.
pub fn data_home() -> PathBuf {
    home_relative("XDG_DATA_HOME", ".local/share")
}

/// Where things that can be fetched again go, honouring `XDG_CACHE_HOME`.
pub fn cache_home() -> PathBuf {
    home_relative("XDG_CACHE_HOME", ".cache")
}

/// Where data installed for everyone goes, honouring `XDG_DATA_DIRS`, in the
/// order it prefers them.
pub fn data_dirs() -> Vec<PathBuf> {
    let set = std::env::var_os("XDG_DATA_DIRS").unwrap_or_default();
    shared(&set.to_string_lossy())
}

/// The listed directories, or what the specification says to use when the
/// list names none. Relative entries are invalid and dropped.
fn shared(set: &str) -> Vec<PathBuf> {
    let listed: Vec<PathBuf> = set
        .split(':')
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .collect();

    if listed.is_empty() {
        return vec![
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share"),
        ];
    }

    listed
}

/// An absolute override wins; anything else falls back under the home
/// directory, and a missing home leaves the relative path to be resolved
/// against wherever aeris was started.
fn home_relative(variable: &str, fallback: &str) -> PathBuf {
    if let Some(set) = std::env::var_os(variable) {
        let path = PathBuf::from(set);
        if path.is_absolute() {
            return path;
        }
    }

    match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home).join(fallback),
        None => PathBuf::from(fallback),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_override_is_ignored() {
        // The specification says a relative value is invalid, and treating it
        // as valid would scatter files wherever the program was started.
        assert!(home_relative("PATH_THAT_IS_NOT_SET_HERE", ".config").is_absolute());
    }

    #[test]
    fn an_unset_list_falls_back_to_what_the_specification_names() {
        assert_eq!(
            shared(""),
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share")
            ]
        );
    }

    #[test]
    fn shared_directories_keep_the_order_they_were_listed_in() {
        assert_eq!(
            shared("/opt/aeris/share:/usr/share"),
            vec![
                PathBuf::from("/opt/aeris/share"),
                PathBuf::from("/usr/share")
            ]
        );
    }

    #[test]
    fn a_relative_entry_is_dropped_rather_than_searched() {
        assert_eq!(
            shared("share:/usr/share"),
            vec![PathBuf::from("/usr/share")]
        );
    }
}
