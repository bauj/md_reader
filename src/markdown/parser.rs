use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, CodeBlockKind};

#[derive(Clone, Debug)]
pub struct ParsedDoc {
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug)]
pub enum Block {
    Heading(u32, Vec<Inline>),
    Paragraph(Vec<Inline>),
    CodeBlock(String, String),       // language, code
    BlockQuote(Vec<Inline>),
    List(bool, Vec<Vec<Inline>>),    // ordered, items
    Table(Vec<String>, Vec<Vec<String>>), // headers, rows
    Rule,
}

#[derive(Clone, Debug)]
pub enum Inline {
    Text(String),
    Bold(String),
    Italic(String),
    BoldItalic(String),
    Code(String),
    Link(String, String), // text, url
}

pub fn parse_markdown(text: &str) -> ParsedDoc {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);

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
    enum Ctx { None, Heading(u32), Paragraph, BlockQuote, ListItem, CodeBlock, TableHead, TableCell }
    let mut ctx = Ctx::None;

    // List state
    let mut list_ordered = false;
    let mut list_items: Vec<Vec<Inline>> = Vec::new();

    // Code block state
    let mut code_lang = String::new();
    let mut code_buf = String::new();

    // Table state
    let mut table_headers: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut table_cur_row: Vec<String> = Vec::new();
    let mut table_cur_cell = String::new();
    let mut table_in_head = false;

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
                current_inlines.clear();
                ctx = Ctx::Paragraph;
            }
            Event::Start(Tag::BlockQuote(_)) => {
                current_inlines.clear();
                ctx = Ctx::BlockQuote;
            }
            Event::Start(Tag::List(order)) => {
                list_ordered = order.is_some();
                list_items.clear();
            }
            Event::Start(Tag::Item) => {
                current_inlines.clear();
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
                table_cur_cell.clear();
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
                if ctx == Ctx::Paragraph || ctx == Ctx::ListItem || ctx == Ctx::BlockQuote {
                    if ctx == Ctx::Paragraph {
                        blocks.push(Block::Paragraph(std::mem::take(&mut current_inlines)));
                    }
                    // for ListItem/BlockQuote the inline content is kept until the item/quote ends
                    ctx = Ctx::None;
                }
            }
            Event::End(TagEnd::BlockQuote(..)) => {
                blocks.push(Block::BlockQuote(std::mem::take(&mut current_inlines)));
                ctx = Ctx::None;
            }
            Event::End(TagEnd::Item) => {
                list_items.push(std::mem::take(&mut current_inlines));
                ctx = Ctx::None;
            }
            Event::End(TagEnd::List(_)) => {
                blocks.push(Block::List(list_ordered, std::mem::take(&mut list_items)));
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
                    table_headers.push(std::mem::take(&mut table_cur_cell));
                } else {
                    table_cur_row.push(std::mem::take(&mut table_cur_cell));
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
                } else if ctx == Ctx::TableCell {
                    table_cur_cell.push_str(&t);
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

            _ => {}
        }
    }

    ParsedDoc { blocks }
}
