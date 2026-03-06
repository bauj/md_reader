pub mod editor_highlight;
pub mod highlight;
pub mod parser;
pub mod renderer;

pub use highlight::Highlighter;
pub use parser::{parse_markdown, Block, Inline, ListItem, ParsedDoc};
pub use renderer::render_markdown;

/// Options that control how the search query is matched.
/// Passed through the renderer so every `find_matches` call respects them.
#[derive(Clone, Copy, Default)]
pub struct SearchOpts {
    pub case_sensitive: bool,
    pub whole_word:     bool,
}
