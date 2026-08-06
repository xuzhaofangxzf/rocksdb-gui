use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "src/asserts"]
#[include = "*.svg"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let custom_path = path.strip_prefix("icons/").unwrap_or(path);
        if let Some(file) = Self::get(custom_path) {
            return Ok(Some(file.data));
        }

        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let custom_prefix = if path.is_empty() { "" } else { "icons/" };
        let mut files = Self::iter()
            .filter_map(|file| {
                let file_path = format!("{custom_prefix}{file}");
                file_path.starts_with(path).then(|| file_path.into())
            })
            .collect::<Vec<_>>();
        files.extend(gpui_component_assets::Assets.list(path)?);
        Ok(files)
    }
}
