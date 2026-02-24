use std::collections::HashMap;
use egui::Color32;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// One highlighted line = a sequence of (color, text-fragment) pairs.
pub type HighlightedLine  = Vec<(Color32, String)>;
pub type HighlightedLines = Vec<HighlightedLine>;

/// Holds syntect resources and a per-(lang, code) cache so we never
/// re-highlight the same block twice within a session.
pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set:  ThemeSet,
    cache:      HashMap<(String, String, bool), HighlightedLines>,
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set:  ThemeSet::load_defaults(),
            cache:      HashMap::new(),
        }
    }

    /// Return highlighted lines for `(lang, code)`, using the cache.
    /// `light_bg` selects a light-friendly syntect theme when `true`.
    pub fn highlight(&mut self, lang: &str, code: &str, light_bg: bool) -> &HighlightedLines {
        let key = (lang.to_string(), code.to_string(), light_bg);
        if !self.cache.contains_key(&key) {
            let lines = Self::do_highlight(&self.syntax_set, &self.theme_set, lang, code, light_bg);
            self.cache.insert(key.clone(), lines);
        }
        self.cache.get(&key).unwrap()
    }

    fn do_highlight(
        ss:       &SyntaxSet,
        ts:       &ThemeSet,
        lang:     &str,
        code:     &str,
        light_bg: bool,
    ) -> HighlightedLines {
        let syntax = if lang.is_empty() {
            ss.find_syntax_plain_text()
        } else {
            ss.find_syntax_by_token(lang)
                .unwrap_or_else(|| ss.find_syntax_plain_text())
        };

        let syntect_theme = if light_bg { "InspiredGitHub" } else { "base16-ocean.dark" };
        let theme = &ts.themes[syntect_theme];
        let mut hl = HighlightLines::new(syntax, theme);

        LinesWithEndings::from(code)
            .map(|line| {
                hl.highlight_line(line, ss)
                    .unwrap_or_default()
                    .iter()
                    .map(|(style, text)| (to_color32(style), (*text).to_string()))
                    .collect()
            })
            .collect()
    }
}

fn to_color32(style: &Style) -> Color32 {
    let c = style.foreground;
    Color32::from_rgb(c.r, c.g, c.b)
}
