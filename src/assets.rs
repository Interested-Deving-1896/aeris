//! The icons drawn from vectors rather than from a font.
//!
//! Compiled in rather than read from disk, so aeris stays the single file it
//! is shipped as.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// Where an icon lives, and the bytes it is.
const ICONS: &[(&str, &[u8])] = &[(
    "icons/close.svg",
    include_bytes!("../assets/icons/close.svg"),
)];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}
