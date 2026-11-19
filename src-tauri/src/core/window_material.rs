//! Window backdrop material (ADR 0002): Mica on Windows 11, opaque elsewhere.
//! The user-facing setting (Mica / Acrylic / Off) is wired in T-71.

use tauri::utils::config::WindowEffectsConfig;
use tauri::window::Effect;

/// Backdrop behind the glass chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    Mica,
    Acrylic,
    Off,
}

impl Material {
    /// Value written to `<html data-material>` so CSS can pick transparent or opaque surfaces.
    pub fn as_str(self) -> &'static str {
        match self {
            Material::Mica => "mica",
            Material::Acrylic => "acrylic",
            Material::Off => "off",
        }
    }

    /// Parses a setting value ("mica" | "acrylic" | "off"), case-insensitive.
    pub fn parse(value: &str) -> Option<Material> {
        match value.trim().to_ascii_lowercase().as_str() {
            "mica" => Some(Material::Mica),
            "acrylic" => Some(Material::Acrylic),
            "off" => Some(Material::Off),
            _ => None,
        }
    }

    /// Requested material: `UNISON_MATERIAL` env var (dev override) or the saved setting.
    pub fn requested(saved: crate::model::WindowMaterial) -> Material {
        std::env::var("UNISON_MATERIAL")
            .ok()
            .and_then(|v| Material::parse(&v))
            .unwrap_or(match saved {
                crate::model::WindowMaterial::Mica => Material::Mica,
                crate::model::WindowMaterial::Acrylic => Material::Acrylic,
                crate::model::WindowMaterial::Off => Material::Off,
            })
    }

    /// Native effect for this material, if any.
    pub fn effects(self) -> Option<WindowEffectsConfig> {
        let effect = match self {
            Material::Mica => Effect::Mica,
            Material::Acrylic => Effect::Acrylic,
            Material::Off => return None,
        };
        Some(WindowEffectsConfig {
            effects: vec![effect],
            state: None,
            radius: None,
            color: None,
            interactive: false,
        })
    }
}

/// First Windows 11 build.
const WIN11_BUILD: u32 = 22_000;
/// Windows 10 1903, first build where Acrylic is usable.
const ACRYLIC_MIN_BUILD: u32 = 18_362;

/// Picks the material to apply: the requested one if this Windows build supports it,
/// otherwise the best fallback. `build` is `None` off Windows.
pub fn resolve(requested: Material, build: Option<u32>) -> Material {
    let Some(build) = build else {
        return Material::Off;
    };
    match requested {
        Material::Mica if build >= WIN11_BUILD => Material::Mica,
        // Mica is unsupported on Windows 10: stay opaque rather than fall back to laggy Acrylic.
        Material::Acrylic if build >= ACRYLIC_MIN_BUILD => Material::Acrylic,
        _ => Material::Off,
    }
}

/// Current Windows build number, `None` on other platforms.
pub fn windows_build() -> Option<u32> {
    #[cfg(windows)]
    {
        Some(windows_version::OsVersion::current().build)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Script injected before the page loads so the first paint already uses the right surfaces.
pub fn init_script(material: Material) -> String {
    format!(
        "document.documentElement.dataset.material = {:?};",
        material.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mica_on_windows_11() {
        assert_eq!(resolve(Material::Mica, Some(22_631)), Material::Mica);
        assert_eq!(resolve(Material::Mica, Some(26_100)), Material::Mica);
    }

    #[test]
    fn mica_falls_back_to_opaque_on_windows_10() {
        assert_eq!(resolve(Material::Mica, Some(19_045)), Material::Off);
    }

    #[test]
    fn acrylic_needs_1903() {
        assert_eq!(resolve(Material::Acrylic, Some(19_045)), Material::Acrylic);
        assert_eq!(resolve(Material::Acrylic, Some(17_763)), Material::Off);
    }

    #[test]
    fn off_stays_off_and_non_windows_is_off() {
        assert_eq!(resolve(Material::Off, Some(26_100)), Material::Off);
        assert_eq!(resolve(Material::Mica, None), Material::Off);
    }

    #[test]
    fn parses_setting_values() {
        assert_eq!(Material::parse(" Acrylic "), Some(Material::Acrylic));
        assert_eq!(Material::parse("OFF"), Some(Material::Off));
        assert_eq!(Material::parse("glass"), None);
    }

    #[test]
    fn effects_and_script() {
        assert!(Material::Off.effects().is_none());
        let fx = Material::Mica.effects().map(|c| c.effects);
        assert_eq!(fx, Some(vec![Effect::Mica]));
        assert_eq!(
            init_script(Material::Mica),
            "document.documentElement.dataset.material = \"mica\";"
        );
    }
}
