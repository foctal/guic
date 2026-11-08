//! Application-owned translations with deterministic English fallbacks.
use gpui::{App, Global, SharedString};
use std::collections::BTreeMap;

/// Translation catalogs shared by GUIC controls and application views.
///
/// Catalog keys are stable semantic identifiers, such as `date.choose`.
/// Lookup tries the full locale, its language, then the supplied English text.
/// Register catalogs before changing the active locale with [`set_locale`].
#[derive(Clone, Debug)]
pub struct Localization {
    locale: SharedString,
    catalogs: BTreeMap<String, BTreeMap<String, SharedString>>,
}
impl Default for Localization {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            catalogs: BTreeMap::new(),
        }
    }
}
impl Global for Localization {}
impl Localization {
    /// Adds or replaces messages for a locale without discarding other messages.
    pub fn register(
        &mut self,
        locale: &str,
        messages: impl IntoIterator<Item = (impl Into<String>, impl Into<SharedString>)>,
    ) {
        self.catalogs.entry(normalize(locale)).or_default().extend(
            messages
                .into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
    }
    /// Returns the active normalized locale identifier.
    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }
    /// Looks up a message, falling back to the language and then English text.
    #[must_use]
    pub fn translate(&self, key: &str, fallback: &str) -> SharedString {
        self.catalogs
            .get(self.locale.as_ref())
            .and_then(|catalog| catalog.get(key))
            .or_else(|| {
                self.locale
                    .split('-')
                    .next()
                    .and_then(|language| self.catalogs.get(language))
                    .and_then(|catalog| catalog.get(key))
            })
            .cloned()
            .unwrap_or_else(|| fallback.to_owned().into())
    }
}
fn normalize(locale: &str) -> String {
    locale.trim().replace('_', "-").to_ascii_lowercase()
}

/// Changes the active locale and refreshes all windows.
///
/// Catalog changes can be made through `cx.global_mut::<Localization>()` after
/// GUIC initialization; call this function afterward to refresh visible text.
pub fn set_locale(locale: &str, cx: &mut App) {
    if !cx.has_global::<Localization>() {
        cx.set_global(Localization::default());
    }
    let locale = normalize(locale);
    cx.global_mut::<Localization>().locale = if locale.is_empty() {
        "en".into()
    } else {
        locale.into()
    };
    cx.refresh_windows();
}

/// Translates a message using the current catalog, or returns its English fallback.
#[must_use]
pub fn translate(cx: &App, key: &str, fallback: &str) -> SharedString {
    if cx.has_global::<Localization>() {
        cx.global::<Localization>().translate(key, fallback)
    } else {
        fallback.to_owned().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogs_use_region_language_and_english_fallbacks() {
        let mut catalog = Localization::default();
        catalog.register("en", [("color", "Color"), ("date.choose", "Choose date")]);
        catalog.register("EN_gb", [("color", "Colour")]);
        catalog.locale = normalize(" EN_GB ").into();
        assert_eq!(catalog.translate("color", "Fallback"), "Colour");
        assert_eq!(catalog.translate("date.choose", "Fallback"), "Choose date");
        assert_eq!(catalog.translate("missing", "Fallback"), "Fallback");
        catalog.register("en-GB", [("other", "Other")]);
        assert_eq!(catalog.translate("color", "Fallback"), "Colour");
    }
}
