//! Story application bootstrapping.

use gpui::App;
use guic::tokens::{Theme, ThemeName, ThemeRegistry};

/// Initializes story-specific registrations.
pub fn init(cx: &mut App) {
    if let Some(base_theme) = ThemeRegistry::global(cx)
        .get(Theme::DEFAULT_DARK_NAME)
        .cloned()
    {
        let mut story_theme = base_theme;
        story_theme.name = ThemeName::new("StoryDark");
        ThemeRegistry::global_mut(cx).register(story_theme);
    }
}
