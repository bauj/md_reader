use std::collections::HashMap;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, CodeBlockKind};

#[derive(Clone, Debug)]
pub struct ParsedDoc {
    pub blocks: Vec<Block>,
    // label -> (display_number, def_block_idx, ref_block_idx)
    pub footnote_map: HashMap<String, (usize, usize, usize)>,
}

#[derive(Clone, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Block {
    Heading(u32, Vec<Inline>),
    Paragraph(Vec<Inline>),
    CodeBlock(String, String),       // language, code
    BlockQuote(Vec<Inline>),
    List(bool, Vec<ListItem>),       // ordered, items
    Table(Vec<Vec<Inline>>, Vec<Vec<Vec<Inline>>>), // headers, rows (each cell is a Vec<Inline>)
    Rule,
    FootnoteDef(String, Vec<Inline>),  // label, content
}

#[derive(Clone, Debug)]
pub struct ListItem {
    pub inlines:  Vec<Inline>,
    pub children: Vec<Block>,   // nested sub-lists
    pub checked:  Option<bool>, // Some(true/false) for task list items, None for regular
}

#[derive(Clone, Debug)]
pub enum Inline {
    Text(String),
    Bold(String),
    Italic(String),
    BoldItalic(String),
    Code(String),
    Link(String, String), // text, url
    Image(String, String), // url, alt text
    FootnoteRef(String),   // label
}

pub fn parse_markdown(text: &str) -> ParsedDoc {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(text, opts);

    let mut blocks: Vec<Block> = Vec::new();

    // Inline state
    let mut bold = false;
    let mut italic = false;
    let mut link_url: Option<String> = None;
    let mut link_text = String::new();
    let mut current_inlines: Vec<Inline> = Vec::new();

    // Block context
    #[derive(PartialEq)]
    enum Ctx { None, Heading(u32), Paragraph, BlockQuote, ListItem, CodeBlock, TableCell, FootnoteDef }
    let mut ctx = Ctx::None;

    // List state — stack to support nested lists
    struct ListFrame {
        ordered:       bool,
        items:         Vec<ListItem>,
        item_children: Vec<Block>,  // sub-blocks for the item currently being parsed
        item_inlines:  Vec<Inline>, // parent item's inlines saved when a sub-list starts
        item_checked:  Option<bool>, // task list marker for the current item being parsed
    }
    let mut list_stack: Vec<ListFrame> = Vec::new();

    // Code block state
    let mut code_lang = String::new();
    let mut code_buf = String::new();

    // Table state
    let mut table_headers: Vec<Vec<Inline>> = Vec::new();
    let mut table_rows: Vec<Vec<Vec<Inline>>> = Vec::new();
    let mut table_cur_row: Vec<Vec<Inline>> = Vec::new();
    let mut table_in_head = false;

    // Footnote state
    let mut footnote_refs: Vec<String> = Vec::new();            // labels in order of first appearance
    let mut footnote_ref_locs: HashMap<String, usize> = HashMap::new(); // label -> approx ref block idx
    let mut footnote_defs: HashMap<String, Vec<Inline>> = HashMap::new(); // label -> content
    let mut footnote_label = String::new();                    // current def label being parsed

    let push_text = |inlines: &mut Vec<Inline>, bold: bool, italic: bool, text: &str| {
        let s = text.to_string();
        inlines.push(match (bold, italic) {
            (true, true)  => Inline::BoldItalic(s),
            (true, false) => Inline::Bold(s),
            (false, true) => Inline::Italic(s),
            _             => Inline::Text(s),
        });
    };

    for event in parser {
        match event {
            // ── Block starts ────────────────────────────────────────────
            Event::Start(Tag::Heading { level, .. }) => {
                current_inlines.clear();
                ctx = Ctx::Heading(level as u32);
            }
            Event::Start(Tag::Paragraph) => {
                if list_stack.is_empty() && ctx != Ctx::BlockQuote && ctx != Ctx::FootnoteDef {
                    current_inlines.clear();
                    ctx = Ctx::Paragraph;
                }
                // inside a list item, blockquote, or footnote def: let inlines keep accumulating
            }
            Event::Start(Tag::BlockQuote(_)) => {
                current_inlines.clear();
                ctx = Ctx::BlockQuote;
            }
            Event::Start(Tag::List(order)) => {
                let saved = if list_stack.is_empty() {
                    Vec::new()
                } else {
                    std::mem::take(&mut current_inlines)
                };
                list_stack.push(ListFrame {
                    ordered:       order.is_some(),
                    items:         Vec::new(),
                    item_children: Vec::new(),
                    item_inlines:  saved,
                    item_checked:  None,
                });
            }
            Event::Start(Tag::Item) => {
                current_inlines.clear();
                if let Some(frame) = list_stack.last_mut() {
                    frame.item_children.clear();
                }
                ctx = Ctx::ListItem;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                code_lang = match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented  => String::new(),
                };
                code_buf.clear();
                ctx = Ctx::CodeBlock;
            }
            Event::Start(Tag::Table(_)) => {
                table_headers.clear();
                table_rows.clear();
            }
            Event::Start(Tag::TableHead) => { table_in_head = true; }
            Event::Start(Tag::TableRow)  => { table_cur_row.clear(); }
            Event::Start(Tag::TableCell) => {
                current_inlines.clear();
                ctx = Ctx::TableCell;
            }

            // ── Block ends ──────────────────────────────────────────────
            Event::End(TagEnd::Heading(..)) => {
                blocks.push(Block::Heading(
                    if let Ctx::Heading(l) = ctx { l } else { 1 },
                    std::mem::take(&mut current_inlines),
                ));
                ctx = Ctx::None;
            }
            Event::End(TagEnd::Paragraph) => {
                if !list_stack.is_empty() {
                    // inlines stay in current_inlines; End(Item) will collect them
                } else if ctx == Ctx::Paragraph {
                    blocks.push(Block::Paragraph(std::mem::take(&mut current_inlines)));
                    ctx = Ctx::None;
                }
                // ctx == BlockQuote: leave inlines and ctx alone; End(BlockQuote) will collect them
            }
            Event::End(TagEnd::BlockQuote(..)) => {
                blocks.push(Block::BlockQuote(std::mem::take(&mut current_inlines)));
                ctx = Ctx::None;
            }
            Event::End(TagEnd::Item) => {
                if let Some(frame) = list_stack.last_mut() {
                    let children = std::mem::take(&mut frame.item_children);
                    frame.items.push(ListItem {
                        inlines:  std::mem::take(&mut current_inlines),
                        children,
                        checked:  frame.item_checked.take(),
                    });
                }
                ctx = Ctx::None;
            }
            Event::End(TagEnd::List(_)) => {
                if let Some(frame) = list_stack.pop() {
                    let list_block = Block::List(frame.ordered, frame.items);
                    if list_stack.is_empty() {
                        blocks.push(list_block);
                    } else {
                        current_inlines = frame.item_inlines;
                        list_stack.last_mut().unwrap().item_children.push(list_block);
                    }
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                blocks.push(Block::CodeBlock(
                    std::mem::take(&mut code_lang),
                    std::mem::take(&mut code_buf),
                ));
                ctx = Ctx::None;
            }
            Event::End(TagEnd::TableCell) => {
                if table_in_head {
                    table_headers.push(std::mem::take(&mut current_inlines));
                } else {
                    table_cur_row.push(std::mem::take(&mut current_inlines));
                }
                ctx = Ctx::None;
            }
            Event::End(TagEnd::TableHead) => { table_in_head = false; }
            Event::End(TagEnd::TableRow)  => {
                if !table_in_head {
                    table_rows.push(std::mem::take(&mut table_cur_row));
                }
            }
            Event::End(TagEnd::Table) => {
                blocks.push(Block::Table(
                    std::mem::take(&mut table_headers),
                    std::mem::take(&mut table_rows),
                ));
            }

            // ── Inline formatting ────────────────────────────────────────
            Event::Start(Tag::Strong)   => { bold = true; }
            Event::End(TagEnd::Strong)  => { bold = false; }
            Event::Start(Tag::Emphasis) => { italic = true; }
            Event::End(TagEnd::Emphasis)=> { italic = false; }
            Event::Start(Tag::Link { dest_url, .. }) => {
                link_url = Some(dest_url.to_string());
                link_text.clear();
            }
            Event::End(TagEnd::Link) => {
                if let Some(url) = link_url.take() {
                    current_inlines.push(Inline::Link(std::mem::take(&mut link_text), url));
                }
            }

            // ── Leaf content ─────────────────────────────────────────────
            Event::Text(t) => {
                if ctx == Ctx::CodeBlock {
                    code_buf.push_str(&t);
                } else if link_url.is_some() {
                    link_text.push_str(&t);
                } else {
                    push_text(&mut current_inlines, bold, italic, &t);
                }
            }
            Event::Code(c) => {
                current_inlines.push(Inline::Code(c.to_string()));
            }
            Event::SoftBreak => {
                if ctx == Ctx::CodeBlock {
                    code_buf.push('\n');
                } else if link_url.is_none() {
                    push_text(&mut current_inlines, false, false, " ");
                }
            }
            Event::HardBreak => {
                if link_url.is_none() {
                    push_text(&mut current_inlines, false, false, "\n");
                }
            }
            Event::Rule => { blocks.push(Block::Rule); }

            // Handle images as inline elements
            Event::Start(Tag::Image { dest_url, .. }) => {
                link_url = Some(dest_url.to_string()); // reuse link_url to hold image URL
                link_text.clear(); // alt text will accumulate here
            }
            Event::End(TagEnd::Image) => {
                if let Some(url) = link_url.take() {
                    let alt_text = std::mem::take(&mut link_text);
                    current_inlines.push(Inline::Image(url, alt_text));
                }
            }

            Event::TaskListMarker(checked) => {
                if let Some(frame) = list_stack.last_mut() {
                    frame.item_checked = Some(checked);
                }
            }

            // ── Footnotes ─────────────────────────────────────────────────
            Event::FootnoteReference(label) => {
                let label = label.to_string();
                if !footnote_ref_locs.contains_key(&label) {
                    footnote_ref_locs.insert(label.clone(), blocks.len());
                    footnote_refs.push(label.clone());
                }
                current_inlines.push(Inline::FootnoteRef(label));
            }
            Event::Start(Tag::FootnoteDefinition(label)) => {
                footnote_label = label.to_string();
                current_inlines.clear();
                ctx = Ctx::FootnoteDef;
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                footnote_defs.insert(
                    std::mem::take(&mut footnote_label),
                    std::mem::take(&mut current_inlines),
                );
                ctx = Ctx::None;
            }

            _ => {}
        }
    }

    // ── Post-loop: build footnote blocks and map ──────────────────────────
    let mut footnote_map = HashMap::new();
    if !footnote_refs.is_empty() {
        blocks.push(Block::Rule);  // visual separator before footnotes
        let base_idx = blocks.len();
        for (i, label) in footnote_refs.iter().enumerate() {
            let number        = i + 1;
            let def_block_idx = base_idx + i;
            let ref_block_idx = footnote_ref_locs.get(label).copied().unwrap_or(0);
            footnote_map.insert(label.clone(), (number, def_block_idx, ref_block_idx));
            let content = footnote_defs.remove(label).unwrap_or_default();
            blocks.push(Block::FootnoteDef(label.clone(), content));
        }
    }

    ParsedDoc { blocks, footnote_map }
}
