use std::io::{BufWriter, Cursor};
use std::path::Path;
use printpdf::*;
use printpdf::path::{PaintMode, WindingOrder};
use crate::markdown::{Block, Inline, ListItem, ParsedDoc};

// ── Page geometry (A4, mm) ────────────────────────────────────────────────────
const PAGE_W:   f32 = 210.0;
const PAGE_H:   f32 = 297.0;
const MARGIN_X: f32 = 22.0;
const MARGIN_Y: f32 = 20.0;
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN_X;

// ── Font sizes (pt) ───────────────────────────────────────────────────────────
const BODY_PT: f32 = 11.0;
const CODE_PT: f32 =  9.5;
const H1_PT:   f32 = 24.0;
const H2_PT:   f32 = 20.0;
const H3_PT:   f32 = 17.0;
const H4_PT:   f32 = 14.5;
const H5_PT:   f32 = 12.5;

// ── Char-width approximations ─────────────────────────────────────────────────
// Proportional font: ~0.50 × font_size pt/char; monospace: ~0.60
const PROP_FACTOR: f32 = 0.50;
const MONO_FACTOR: f32 = 0.60;

fn pt_to_mm(pt: f32) -> f32 { pt / 2.835 }

fn char_w_mm(size_pt: f32, mono: bool) -> f32 {
    pt_to_mm(size_pt * if mono { MONO_FACTOR } else { PROP_FACTOR })
}

fn text_w_mm(s: &str, size_pt: f32, mono: bool) -> f32 {
    s.chars().count() as f32 * char_w_mm(size_pt, mono)
}

fn line_h_mm(size_pt: f32) -> f32 {
    pt_to_mm(size_pt * 1.45)
}

// ── Styled word ───────────────────────────────────────────────────────────────
#[derive(Clone)]
struct Word {
    text:  String,
    bold:  bool,
    mono:  bool,
    color: (f32, f32, f32),
    size:  f32,
}

impl Word {
    fn width(&self) -> f32 { text_w_mm(&self.text, self.size, self.mono) }
    fn space(&self) -> f32 { char_w_mm(self.size, self.mono) }
}

// ── Core renderer ─────────────────────────────────────────────────────────────
struct Renderer {
    doc:     PdfDocumentReference,
    layer:   PdfLayerReference,
    regular: IndirectFontRef,
    bold:    IndirectFontRef,
    mono:    IndirectFontRef,
    /// Y cursor in mm from page bottom. Decreases as we write downward.
    y: f32,
}

impl Renderer {
    fn new(title: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let (doc, page1, layer1) =
            PdfDocument::new(title, Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
        let layer = doc.get_page(page1).get_layer(layer1);

        let regular_bytes = include_bytes!("../assets/fonts/Rubik-Regular.ttf");
        let bold_bytes    = include_bytes!("../assets/fonts/Rubik-Bold.ttf");
        let mono_bytes    = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

        let regular = doc.add_external_font(Cursor::new(regular_bytes.as_ref()))?;
        let bold    = doc.add_external_font(Cursor::new(bold_bytes.as_ref()))?;
        let mono    = doc.add_external_font(Cursor::new(mono_bytes.as_ref()))?;

        Ok(Self { doc, layer, regular, bold, mono, y: PAGE_H - MARGIN_Y })
    }

    // ── Page management ───────────────────────────────────────────────────────

    fn ensure(&mut self, h: f32) {
        if self.y - h < MARGIN_Y {
            self.new_page();
        }
    }

    fn new_page(&mut self) {
        let (page, layer1) = self.doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
        self.layer = self.doc.get_page(page).get_layer(layer1);
        self.y = PAGE_H - MARGIN_Y;
    }

    // ── Drawing primitives ────────────────────────────────────────────────────

    fn set_fill(&self, r: f32, g: f32, b: f32) {
        self.layer.set_fill_color(Color::Rgb(Rgb { r, g, b, icc_profile: None }));
    }

    fn set_stroke(&self, r: f32, g: f32, b: f32, thickness: f32) {
        self.layer.set_outline_color(Color::Rgb(Rgb { r, g, b, icc_profile: None }));
        self.layer.set_outline_thickness(thickness);
    }

    fn font_ref(&self, bold: bool, mono: bool) -> &IndirectFontRef {
        if mono { &self.mono } else if bold { &self.bold } else { &self.regular }
    }

    fn put_text(&self, text: &str, size: f32, x: f32, y: f32, bold: bool, mono: bool) {
        let font = self.font_ref(bold, mono);
        self.layer.use_text(text, size, Mm(x), Mm(y), font);
    }

    fn filled_rect(&self, x: f32, y_bottom: f32, w: f32, h: f32, r: f32, g: f32, b: f32) {
        self.set_fill(r, g, b);
        let pts = vec![
            vec![
                (Point::new(Mm(x),       Mm(y_bottom)),       false),
                (Point::new(Mm(x + w),   Mm(y_bottom)),       false),
                (Point::new(Mm(x + w),   Mm(y_bottom + h)),   false),
                (Point::new(Mm(x),       Mm(y_bottom + h)),   false),
            ]
        ];
        self.layer.add_polygon(Polygon {
            rings: pts,
            mode: PaintMode::Fill,
            winding_order: WindingOrder::NonZero,
        });
    }

    fn hline(&self, x1: f32, x2: f32, y: f32, r: f32, g: f32, b: f32, thickness: f32) {
        self.set_stroke(r, g, b, thickness);
        self.layer.add_line(Line {
            points: vec![
                (Point::new(Mm(x1), Mm(y)), false),
                (Point::new(Mm(x2), Mm(y)), false),
            ],
            is_closed: false,
        });
    }

    fn vline(&self, x: f32, y1: f32, y2: f32, r: f32, g: f32, b: f32, thickness: f32) {
        self.set_stroke(r, g, b, thickness);
        self.layer.add_line(Line {
            points: vec![
                (Point::new(Mm(x), Mm(y1)), false),
                (Point::new(Mm(x), Mm(y2)), false),
            ],
            is_closed: false,
        });
    }

    // ── Block renderers ───────────────────────────────────────────────────────

    fn render_block(&mut self, block: &Block, x_indent: f32) {
        match block {
            Block::Heading(level, inlines)   => self.render_heading(*level, inlines),
            Block::Paragraph(inlines)        => self.render_paragraph(inlines, x_indent, BODY_PT),
            Block::CodeBlock(_, code)        => self.render_code_block(code),
            Block::BlockQuote(inlines)       => self.render_blockquote(inlines),
            Block::List(ordered, items)      => self.render_list(*ordered, items, x_indent),
            Block::Table(headers, rows)      => self.render_table(headers, rows),
            Block::Rule                      => self.render_rule(),
        }
    }

    fn render_heading(&mut self, level: u32, inlines: &[Inline]) {
        let size = match level { 1 => H1_PT, 2 => H2_PT, 3 => H3_PT, 4 => H4_PT, _ => H5_PT };
        let gap_above = match level { 1 => 7.0_f32, 2 => 5.0, _ => 3.5 };
        let gap_below = match level { 1 => 2.5_f32, 2 => 2.0, _ => 1.5 };
        let lh = line_h_mm(size);

        self.ensure(gap_above + lh + gap_below + 2.0);
        self.y -= gap_above;
        self.y -= lh;

        let text = inlines_to_plain(inlines);
        self.set_fill(0.08, 0.08, 0.08);
        self.put_text(&text, size, MARGIN_X, self.y, true, false);

        self.y -= gap_below;

        if level <= 2 {
            self.hline(MARGIN_X, MARGIN_X + CONTENT_W, self.y, 0.75, 0.75, 0.75, 0.4);
            self.y -= 1.5;
        }
    }

    fn render_paragraph(&mut self, inlines: &[Inline], x_indent: f32, size: f32) {
        let words = inlines_to_words(inlines, size);
        if words.is_empty() { return; }

        let lh = line_h_mm(size);
        let avail = CONTENT_W - x_indent;
        let lines = word_wrap(&words, avail);
        if lines.is_empty() { return; }

        self.ensure(lh);
        self.y -= lh * 0.15;

        for line_words in &lines {
            self.ensure(lh);
            self.y -= lh;
            let mut x = MARGIN_X + x_indent;
            for (wi, word) in line_words.iter().enumerate() {
                self.set_fill(word.color.0, word.color.1, word.color.2);
                self.put_text(&word.text, word.size, x, self.y, word.bold, word.mono);
                x += word.width();
                if wi + 1 < line_words.len() {
                    x += word.space();
                }
            }
        }
        self.y -= lh * 0.4;
    }

    fn render_code_block(&mut self, code: &str) {
        let code_lines: Vec<&str> = code.lines().collect();
        if code_lines.is_empty() { return; }

        let lh  = line_h_mm(CODE_PT);
        let pad = 2.5_f32;

        self.ensure(lh + pad * 2.0);
        self.y -= pad;

        for line_text in &code_lines {
            self.ensure(lh);
            // background behind this line
            let rect_bottom = self.y - lh - pad * 0.5;
            self.filled_rect(MARGIN_X, rect_bottom, CONTENT_W, lh + pad * 0.8, 0.93, 0.93, 0.93);
            self.y -= lh;
            self.set_fill(0.1, 0.1, 0.1);
            self.put_text(line_text, CODE_PT, MARGIN_X + 3.0, self.y, false, true);
        }
        self.y -= pad + 2.0;
    }

    fn render_blockquote(&mut self, inlines: &[Inline]) {
        let words = inlines_to_words(inlines, BODY_PT);
        let lh    = line_h_mm(BODY_PT);
        let indent = 8.0_f32;
        let lines = word_wrap(&words, CONTENT_W - indent);
        if lines.is_empty() { return; }

        self.ensure(lh);
        let top_y = self.y;
        self.y -= lh * 0.15;

        for line_words in &lines {
            self.ensure(lh);
            self.y -= lh;
            let mut x = MARGIN_X + indent;
            for (wi, word) in line_words.iter().enumerate() {
                self.set_fill(0.40, 0.40, 0.40);
                self.put_text(&word.text, word.size, x, self.y, word.bold, word.mono);
                x += word.width();
                if wi + 1 < line_words.len() { x += word.space(); }
            }
        }
        let bottom_y = self.y;
        self.y -= lh * 0.4;

        // Left accent bar
        self.vline(MARGIN_X + 2.0, bottom_y, top_y, 0.55, 0.55, 0.78, 2.0);
    }

    fn render_list(&mut self, ordered: bool, items: &[ListItem], x_indent: f32) {
        let lh = line_h_mm(BODY_PT);
        let item_indent = x_indent + 7.0;

        for (i, item) in items.iter().enumerate() {
            let bullet = if ordered { format!("{}.", i + 1) } else { "•".to_string() };
            let words  = inlines_to_words(&item.inlines, BODY_PT);
            let lines  = word_wrap(&words, CONTENT_W - item_indent);

            // First line: bullet + text on same y
            self.ensure(lh);
            self.y -= lh;
            self.set_fill(0.40, 0.40, 0.40);
            self.put_text(&bullet, BODY_PT, MARGIN_X + x_indent, self.y, false, false);

            if let Some(first) = lines.first() {
                let mut x = MARGIN_X + item_indent;
                for (wi, word) in first.iter().enumerate() {
                    self.set_fill(word.color.0, word.color.1, word.color.2);
                    self.put_text(&word.text, word.size, x, self.y, word.bold, word.mono);
                    x += word.width();
                    if wi + 1 < first.len() { x += word.space(); }
                }
            }

            // Continuation lines
            for line_words in lines.iter().skip(1) {
                self.ensure(lh);
                self.y -= lh;
                let mut x = MARGIN_X + item_indent;
                for (wi, word) in line_words.iter().enumerate() {
                    self.set_fill(word.color.0, word.color.1, word.color.2);
                    self.put_text(&word.text, word.size, x, self.y, word.bold, word.mono);
                    x += word.width();
                    if wi + 1 < line_words.len() { x += word.space(); }
                }
            }

            // Nested children (sub-lists)
            for child in &item.children {
                self.render_block(child, item_indent);
            }

            self.y -= 1.0;
        }
        self.y -= 2.0;
    }

    fn render_table(&mut self, headers: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) {
        if headers.is_empty() { return; }

        let n_cols = headers.len();
        let col_w  = CONTENT_W / n_cols as f32;
        let lh     = line_h_mm(BODY_PT);
        let row_h  = lh + 2.5;

        self.ensure(row_h * 2.0);
        self.y -= 2.0;

        // Header row
        let header_bottom = self.y - row_h;
        self.filled_rect(MARGIN_X, header_bottom, CONTENT_W, row_h, 0.88, 0.88, 0.92);
        self.y -= row_h;
        for (c, cell) in headers.iter().enumerate() {
            let text = inlines_to_plain(cell);
            self.set_fill(0.08, 0.08, 0.08);
            self.put_text(&text, BODY_PT, MARGIN_X + c as f32 * col_w + 2.0, self.y + 1.5, true, false);
        }
        self.hline(MARGIN_X, MARGIN_X + CONTENT_W, self.y, 0.5, 0.5, 0.55, 0.5);

        // Data rows
        for row in rows {
            self.ensure(row_h);
            self.y -= row_h;
            for (c, cell) in row.iter().enumerate().take(n_cols) {
                let text = inlines_to_plain(cell);
                self.set_fill(0.10, 0.10, 0.10);
                self.put_text(&text, BODY_PT, MARGIN_X + c as f32 * col_w + 2.0, self.y + 1.5, false, false);
            }
            self.hline(MARGIN_X, MARGIN_X + CONTENT_W, self.y, 0.80, 0.80, 0.80, 0.2);
        }

        // Outer border
        self.hline(MARGIN_X, MARGIN_X + CONTENT_W, self.y, 0.50, 0.50, 0.55, 0.4);
        self.y -= 3.0;
    }

    fn render_rule(&mut self) {
        self.ensure(6.0);
        self.y -= 3.0;
        self.hline(MARGIN_X, MARGIN_X + CONTENT_W, self.y, 0.70, 0.70, 0.70, 0.5);
        self.y -= 3.0;
    }
}

// ── Inline helpers ────────────────────────────────────────────────────────────

fn inlines_to_plain(inlines: &[Inline]) -> String {
    inlines.iter().map(|il| match il {
        Inline::Text(s) | Inline::Bold(s) | Inline::Italic(s)
        | Inline::BoldItalic(s) | Inline::Code(s) => s.as_str(),
        Inline::Link(t, _)  => t.as_str(),
        Inline::Image(_, a) => a.as_str(),
    }).collect()
}

fn inlines_to_words(inlines: &[Inline], default_size: f32) -> Vec<Word> {
    let mut out = Vec::new();
    for inline in inlines {
        let (text, bold, mono, color): (&str, bool, bool, (f32, f32, f32)) = match inline {
            Inline::Text(s)       => (s, false, false, (0.10, 0.10, 0.10)),
            Inline::Bold(s)       => (s, true,  false, (0.05, 0.05, 0.05)),
            Inline::Italic(s)     => (s, false, false, (0.15, 0.15, 0.15)),
            Inline::BoldItalic(s) => (s, true,  false, (0.05, 0.05, 0.05)),
            Inline::Code(s)       => (s, false, true,  (0.12, 0.12, 0.12)),
            Inline::Link(t, _)    => (t, false, false, (0.17, 0.47, 0.64)),
            Inline::Image(_, a)   => (a, false, false, (0.50, 0.50, 0.50)),
        };
        let size = if mono { CODE_PT } else { default_size };
        for raw in text.split_whitespace() {
            out.push(Word { text: raw.to_string(), bold, mono, color, size });
        }
    }
    out
}

fn word_wrap(words: &[Word], max_w: f32) -> Vec<Vec<Word>> {
    let mut lines: Vec<Vec<Word>> = Vec::new();
    let mut cur:   Vec<Word>      = Vec::new();
    let mut cur_w = 0.0_f32;

    for word in words {
        let w     = word.width();
        let space = if cur.is_empty() { 0.0 } else { word.space() };
        if !cur.is_empty() && cur_w + space + w > max_w {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0.0;
        }
        cur_w += space + w;
        cur.push(word.clone());
    }
    if !cur.is_empty() { lines.push(cur); }
    lines
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn export_pdf(doc: &ParsedDoc, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let title = dest.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Document".to_string());

    let mut r = Renderer::new(&title)?;

    for block in &doc.blocks {
        r.render_block(block, 0.0);
    }

    let file = std::fs::File::create(dest)?;
    r.doc.save(&mut BufWriter::new(file))?;
    Ok(())
}
