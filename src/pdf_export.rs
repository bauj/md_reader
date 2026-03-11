use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use printpdf::*;
use printpdf::path::{PaintMode, WindingOrder};
use crate::markdown::{Block, Inline, ListItem, ParsedDoc, Highlighter};

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

fn pt_to_mm(pt: f32) -> f32 { pt / 2.835 }

fn line_h_mm(size_pt: f32) -> f32 {
    pt_to_mm(size_pt * 1.45)
}

// ── Font bytes (static so they can be shared by printpdf and ttf-parser) ─────
static REGULAR_BYTES:   &[u8] = include_bytes!("../assets/fonts/Rubik-Regular.ttf");
static BOLD_BYTES:      &[u8] = include_bytes!("../assets/fonts/Rubik-Bold.ttf");
static ITALIC_BYTES:    &[u8] = include_bytes!("../assets/fonts/NotoSans-Italic.ttf");
static BOLDITAL_BYTES:  &[u8] = include_bytes!("../assets/fonts/NotoSans-BoldItalic.ttf");
static MONO_BYTES:      &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

// ── Real font metrics via ttf-parser ─────────────────────────────────────────

/// Measure one character's advance width, trying `primary` first then `fallback`.
fn char_advance(
    primary:  &ttf_parser::Face<'static>,
    fallback: &ttf_parser::Face<'static>,
    c: char,
    size_pt: f32,
) -> f32 {
    let (face, use_fb) = if primary.glyph_index(c).is_some() {
        (primary, false)
    } else {
        (fallback, true)
    };
    let _ = use_fb;
    let upm   = face.units_per_em() as f32;
    let scale = size_pt / (upm * 2.835_f32);
    face.glyph_index(c)
        .and_then(|g| face.glyph_hor_advance(g))
        .map(|a| a as f32 * scale)
        .unwrap_or(upm * 0.55 * scale)
}

/// Measure `text` at `size_pt`, using `fallback` for characters absent from `primary`.
fn face_measure_fb(
    primary:  &ttf_parser::Face<'static>,
    fallback: &ttf_parser::Face<'static>,
    text:     &str,
    size_pt:  f32,
) -> f32 {
    text.chars().map(|c| char_advance(primary, fallback, c, size_pt)).sum()
}

/// Measure `text` using only `face`, falling back to 0.55×em for missing glyphs.
fn face_measure(face: &ttf_parser::Face<'static>, text: &str, size_pt: f32) -> f32 {
    face_measure_fb(face, face, text, size_pt)
}

/// Holds one parsed `Face<'static>` per font variant.
struct FontFaces {
    regular:   ttf_parser::Face<'static>,
    bold:      ttf_parser::Face<'static>,
    italic:    ttf_parser::Face<'static>,
    bold_ital: ttf_parser::Face<'static>,
    mono:      ttf_parser::Face<'static>,
}

impl FontFaces {
    fn new() -> Self {
        Self {
            regular:   ttf_parser::Face::parse(REGULAR_BYTES,  0).expect("Rubik-Regular"),
            bold:      ttf_parser::Face::parse(BOLD_BYTES,     0).expect("Rubik-Bold"),
            italic:    ttf_parser::Face::parse(ITALIC_BYTES,   0).expect("NotoSans-Italic"),
            bold_ital: ttf_parser::Face::parse(BOLDITAL_BYTES, 0).expect("NotoSans-BoldItalic"),
            mono:      ttf_parser::Face::parse(MONO_BYTES,     0).expect("JetBrainsMono"),
        }
    }

    fn face(&self, bold: bool, italic: bool, mono: bool) -> &ttf_parser::Face<'static> {
        if mono              { &self.mono }
        else if bold && italic { &self.bold_ital }
        else if bold           { &self.bold }
        else if italic         { &self.italic }
        else                   { &self.regular }
    }

    fn measure(&self, text: &str, size_pt: f32, bold: bool, italic: bool, mono: bool) -> f32 {
        // Use JetBrainsMono as fallback for chars absent from the primary face
        // (e.g. → U+2192 which Rubik lacks).
        face_measure_fb(self.face(bold, italic, mono), &self.mono, text, size_pt)
    }

    /// Width of a single space in the regular body font at `size_pt`.
    fn space_w(&self, size_pt: f32) -> f32 {
        face_measure(&self.regular, " ", size_pt)
    }
}

// ── Styled word ───────────────────────────────────────────────────────────────
#[derive(Clone)]
struct Word {
    text:     String,
    bold:     bool,
    italic:   bool,
    mono:     bool,
    color:    (f32, f32, f32),
    size:     f32,
    /// Pre-measured advance width of `text` in mm.
    width_mm: f32,
    /// Width of the inter-word space that follows this word (in mm).
    spacing:  f32,
}

impl Word {
    fn width(&self) -> f32 { self.width_mm }
}

// ── Core renderer ─────────────────────────────────────────────────────────────
struct Renderer {
    doc:        PdfDocumentReference,
    layer:      PdfLayerReference,
    regular:    IndirectFontRef,
    bold:       IndirectFontRef,
    italic:     IndirectFontRef,
    bold_italic: IndirectFontRef,
    mono:       IndirectFontRef,
    fonts:          FontFaces,
    footnote_nums:  HashMap<String, usize>,
    highlighter:    Highlighter,
    /// Y cursor in mm from page bottom. Decreases as we write downward.
    y: f32,
    /// 0-based page counter, incremented on each new_page call.
    page_num: usize,
    /// Headings as (level, page_num_0based, title) for the PDF outline.
    bookmarks: Vec<(u32, usize, String)>,
}

impl Renderer {
    fn new(title: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let (doc, page1, layer1) =
            PdfDocument::new(title, Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
        let layer = doc.get_page(page1).get_layer(layer1);

        let regular     = doc.add_external_font(Cursor::new(REGULAR_BYTES))?;
        let bold        = doc.add_external_font(Cursor::new(BOLD_BYTES))?;
        let italic      = doc.add_external_font(Cursor::new(ITALIC_BYTES))?;
        let bold_italic = doc.add_external_font(Cursor::new(BOLDITAL_BYTES))?;
        let mono        = doc.add_external_font(Cursor::new(MONO_BYTES))?;

        Ok(Self {
            doc, layer, regular, bold, italic, bold_italic, mono,
            fonts:       FontFaces::new(),
            footnote_nums: HashMap::new(),
            highlighter: Highlighter::new(),
            y: PAGE_H - MARGIN_Y,
            page_num: 0,
            bookmarks: Vec::new(),
        })
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
        self.page_num += 1;
    }

    // ── Drawing primitives ────────────────────────────────────────────────────

    fn set_fill(&self, r: f32, g: f32, b: f32) {
        self.layer.set_fill_color(Color::Rgb(Rgb { r, g, b, icc_profile: None }));
    }

    fn set_stroke(&self, r: f32, g: f32, b: f32, thickness: f32) {
        self.layer.set_outline_color(Color::Rgb(Rgb { r, g, b, icc_profile: None }));
        self.layer.set_outline_thickness(thickness);
    }

    fn font_ref(&self, bold: bool, italic: bool, mono: bool) -> &IndirectFontRef {
        if mono   { &self.mono }
        else if bold && italic { &self.bold_italic }
        else if bold           { &self.bold }
        else if italic         { &self.italic }
        else                   { &self.regular }
    }

    fn put_text(&self, text: &str, size: f32, x: f32, y: f32, bold: bool, italic: bool, mono: bool) {
        let font = self.font_ref(bold, italic, mono);
        self.layer.use_text(text, size, Mm(x), Mm(y), font);
    }

    /// Like `put_text` but falls back to the mono font for characters absent
    /// from the primary face (e.g. → U+2192 missing from Rubik).
    fn put_text_fb(&self, text: &str, size: f32, x: f32, y: f32, bold: bool, italic: bool, mono: bool) {
        if mono {
            self.put_text(text, size, x, y, bold, italic, mono);
            return;
        }
        let primary = self.fonts.face(bold, italic, mono);
        let mut seg  = String::new();
        let mut in_fb = false;

        let flush = |seg: &str, x: f32, use_fb: bool, s: &Renderer| {
            let (b, i, m) = if use_fb { (false, false, true) } else { (bold, italic, false) };
            s.put_text(seg, size, x, y, b, i, m);
        };

        let mut cx = x;
        for c in text.chars() {
            let needs_fb = primary.glyph_index(c).is_none();
            if needs_fb != in_fb {
                if !seg.is_empty() {
                    flush(&seg, cx, in_fb, self);
                    cx += face_measure_fb(
                        self.fonts.face(
                            if in_fb { false } else { bold },
                            if in_fb { false } else { italic },
                            in_fb,
                        ),
                        &self.fonts.mono,
                        &seg, size,
                    );
                    seg.clear();
                }
                in_fb = needs_fb;
            }
            seg.push(c);
        }
        if !seg.is_empty() {
            flush(&seg, cx, in_fb, self);
        }
        let _ = x; // x was used via cx
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
            Block::CodeBlock(lang, code)     => self.render_code_block(lang, code),
            Block::BlockQuote(inlines)       => self.render_blockquote(inlines),
            Block::List(ordered, items)      => self.render_list(*ordered, items, x_indent),
            Block::Table(headers, rows)      => self.render_table(headers, rows),
            Block::Rule                      => self.render_rule(),
            Block::FootnoteDef(label, inlines) => self.render_footnote_def(label, inlines),
        }
    }

    fn render_heading(&mut self, level: u32, inlines: &[Inline]) {
        // Record this heading for the PDF outline (first heading per page wins).
        let plain: String = inlines.iter().map(|il| match il {
            Inline::Text(t) | Inline::Bold(t) | Inline::Italic(t) | Inline::BoldItalic(t) => t.as_str(),
            Inline::Code(c) => c.as_str(),
            Inline::Link(text, _) => text.as_str(),
            _ => "",
        }).collect::<Vec<_>>().join("");
        if !plain.trim().is_empty() {
            self.bookmarks.push((level, self.page_num, plain));
        }

        let size      = match level { 1 => H1_PT, 2 => H2_PT, 3 => H3_PT, 4 => H4_PT, _ => H5_PT };
        let gap_above = match level { 1 => 7.0_f32, 2 => 5.0, _ => 3.5 };
        let gap_below = match level { 1 => 2.5_f32, 2 => 2.0, _ => 1.5 };
        let lh        = line_h_mm(size);

        // Force bold on all heading words and re-measure widths with the bold face.
        let words: Vec<Word> = inlines_to_words(inlines, size, &self.fonts, &self.footnote_nums)
            .into_iter()
            .map(|mut w| {
                if !w.bold {
                    w.bold     = true;
                    w.width_mm = self.fonts.measure(&w.text, w.size, true, w.italic, w.mono);
                }
                w
            })
            .collect();

        let lines = word_wrap(&words, CONTENT_W);
        if lines.is_empty() { return; }

        let total_text_h = lines.len() as f32 * lh;
        self.ensure(gap_above + total_text_h + gap_below + 2.0);
        self.y -= gap_above;

        self.set_fill(0.08, 0.08, 0.08);
        for line in &lines {
            self.y -= lh;
            let mut x = MARGIN_X;
            for (wi, word) in line.iter().enumerate() {
                self.put_text_fb(&word.text, word.size, x, self.y, word.bold, word.italic, word.mono);
                x += word.width();
                if wi + 1 < line.len() { x += word.spacing; }
            }
        }

        self.y -= gap_below;

        if level <= 2 {
            self.hline(MARGIN_X, MARGIN_X + CONTENT_W, self.y, 0.75, 0.75, 0.75, 0.4);
            self.y -= 1.5;
        }
    }

    fn render_paragraph(&mut self, inlines: &[Inline], x_indent: f32, size: f32) {
        let words = inlines_to_words(inlines, size, &self.fonts, &self.footnote_nums);
        self.render_word_list(&words, x_indent, size);
    }

    fn render_word_list(&mut self, words: &[Word], x_indent: f32, size: f32) {
        if words.is_empty() { return; }

        let lh = line_h_mm(size);
        let avail = CONTENT_W - x_indent;
        let lines = word_wrap(words, avail);
        if lines.is_empty() { return; }

        self.ensure(lh);
        self.y -= lh * 0.15;

        for line_words in &lines {
            self.ensure(lh);
            self.y -= lh;
            let mut x = MARGIN_X + x_indent;
            for (wi, word) in line_words.iter().enumerate() {
                self.set_fill(word.color.0, word.color.1, word.color.2);
                self.put_text_fb(&word.text, word.size, x, self.y, word.bold, word.italic, word.mono);
                x += word.width();
                if wi + 1 < line_words.len() {
                    x += word.spacing;
                }
            }
        }
        self.y -= lh * 0.4;
    }

    fn render_footnote_def(&mut self, label: &str, inlines: &[Inline]) {
        let n = self.footnote_nums.get(label).copied().unwrap_or(0);
        let prefix = format!("[{n}]");
        let spacing = self.fonts.space_w(BODY_PT);
        let mut words = vec![Word {
            width_mm: self.fonts.measure(&prefix, BODY_PT, true, false, false),
            text:     prefix,
            bold:     true,
            italic:   false,
            mono:     false,
            color:    (0.10, 0.10, 0.10),
            size:     BODY_PT,
            spacing,
        }];
        words.extend(inlines_to_words(inlines, BODY_PT, &self.fonts, &self.footnote_nums));
        self.render_word_list(&words, 0.0, BODY_PT);
    }

    fn render_code_block(&mut self, lang: &str, code: &str) {
        if code.lines().next().is_none() { return; }

        let lh         = line_h_mm(CODE_PT);
        let top_pad    = 3.0_f32;
        let bot_pad    = 3.0_f32;
        let h_pad      = 3.0_f32;
        let avail      = CONTENT_W - h_pad * 2.0;
        let margin_top = 2.5_f32;

        // JetBrainsMono is monospace — measure one char to get the per-char budget.
        let char_w         = self.fonts.measure("a", CODE_PT, false, false, true);
        let chars_per_line = if char_w > 0.0 { (avail / char_w).floor() as usize } else { usize::MAX };

        // Get syntax-highlighted tokens: one Vec<(Color32, String)> per source line.
        let highlighted = self.highlighter.highlight(lang, code, true).clone();

        // Flatten each source line's tokens into (r,g,b,char) tuples, then chunk
        // at chars_per_line and re-group into colored spans → sub-lines.
        type Span    = (f32, f32, f32, String);
        type SubLine = Vec<Span>;
        let mut sub_lines: Vec<SubLine> = Vec::new();

        for token_line in &highlighted {
            // Flatten tokens to individual (color, char) pairs.
            let mut char_colors: Vec<(f32, f32, f32, char)> = Vec::new();
            for (color, text) in token_line {
                let (r, g, b) = (
                    color.r() as f32 / 255.0,
                    color.g() as f32 / 255.0,
                    color.b() as f32 / 255.0,
                );
                for c in text.chars().filter(|c| *c != '\n' && *c != '\r') {
                    char_colors.push((r, g, b, c));
                }
            }
            if char_colors.is_empty() {
                sub_lines.push(vec![]);
                continue;
            }
            // Split into sub-lines of chars_per_line, then re-group into spans.
            for chunk in char_colors.chunks(chars_per_line.max(1)) {
                let mut spans: SubLine = Vec::new();
                let mut cur = String::new();
                let mut cr = -1.0f32;
                let mut cg = -1.0f32;
                let mut cb = -1.0f32;
                for &(r, g, b, c) in chunk {
                    if r != cr || g != cg || b != cb {
                        if !cur.is_empty() {
                            spans.push((cr, cg, cb, std::mem::take(&mut cur)));
                        }
                        cr = r; cg = g; cb = b;
                    }
                    cur.push(c);
                }
                if !cur.is_empty() { spans.push((cr, cg, cb, cur)); }
                sub_lines.push(spans);
            }
        }

        let n       = sub_lines.len() as f32;
        let total_h = n * lh + top_pad + bot_pad;
        let fits    = total_h <= PAGE_H - MARGIN_Y * 2.0;

        self.y -= margin_top;

        // Render a single sub-line of colored spans at the current y.
        let render_spans = |layer: &Renderer, spans: &SubLine, y: f32| {
            let mut x = MARGIN_X + h_pad;
            for (r, g, b, text) in spans {
                layer.set_fill(*r, *g, *b);
                layer.put_text(text, CODE_PT, x, y, false, false, true);
                x += text.chars().count() as f32 * char_w;
            }
        };

        if fits {
            self.ensure(total_h);
            let rect_top = self.y;
            self.filled_rect(MARGIN_X, rect_top - total_h, CONTENT_W, total_h, 0.93, 0.93, 0.93);
            self.y -= top_pad;
            for spans in &sub_lines {
                self.y -= lh;
                render_spans(self, spans, self.y);
            }
            self.y -= bot_pad + 1.5;
        } else {
            self.ensure(lh + top_pad + bot_pad);
            self.y -= top_pad;
            for spans in &sub_lines {
                self.ensure(lh + bot_pad);
                self.y -= lh;
                let descent = lh * 0.25;
                self.filled_rect(MARGIN_X, self.y - descent, CONTENT_W, lh, 0.93, 0.93, 0.93);
                render_spans(self, spans, self.y);
            }
            self.y -= bot_pad + 1.5;
        }
    }

    fn render_blockquote(&mut self, inlines: &[Inline]) {
        let words = inlines_to_words(inlines, BODY_PT, &self.fonts, &self.footnote_nums);
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
                self.put_text_fb(&word.text, word.size, x, self.y, word.bold, word.italic, word.mono);
                x += word.width();
                if wi + 1 < line_words.len() { x += word.spacing; }
            }
        }
        let bottom_y = self.y;
        self.y -= lh * 0.4;

        self.vline(MARGIN_X + 2.0, bottom_y, top_y, 0.55, 0.55, 0.78, 2.0);
    }

    fn render_list(&mut self, ordered: bool, items: &[ListItem], x_indent: f32) {
        let lh = line_h_mm(BODY_PT);
        let item_indent = x_indent + 7.0;

        for (i, item) in items.iter().enumerate() {
            let bullet = if ordered { format!("{}.", i + 1) } else { "•".to_string() };
            let words  = inlines_to_words(&item.inlines, BODY_PT, &self.fonts, &self.footnote_nums);
            let lines  = word_wrap(&words, CONTENT_W - item_indent);

            self.ensure(lh);
            self.y -= lh;
            self.set_fill(0.40, 0.40, 0.40);
            self.put_text(&bullet, BODY_PT, MARGIN_X + x_indent, self.y, false, false, false);

            if let Some(first) = lines.first() {
                let mut x = MARGIN_X + item_indent;
                for (wi, word) in first.iter().enumerate() {
                    self.set_fill(word.color.0, word.color.1, word.color.2);
                    self.put_text_fb(&word.text, word.size, x, self.y, word.bold, word.italic, word.mono);
                    x += word.width();
                    if wi + 1 < first.len() { x += word.spacing; }
                }
            }

            for line_words in lines.iter().skip(1) {
                self.ensure(lh);
                self.y -= lh;
                let mut x = MARGIN_X + item_indent;
                for (wi, word) in line_words.iter().enumerate() {
                    self.set_fill(word.color.0, word.color.1, word.color.2);
                    self.put_text_fb(&word.text, word.size, x, self.y, word.bold, word.italic, word.mono);
                    x += word.width();
                    if wi + 1 < line_words.len() { x += word.spacing; }
                }
            }

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
        let h_pad  = 3.0_f32; // horizontal cell padding
        let v_pad  = 2.0_f32; // vertical cell padding (top + bottom)
        let cell_w = col_w - h_pad * 2.0;

        // Wrap inlines for a cell and return lines of words.
        let wrap_cell = |inlines: &Vec<Inline>, fonts: &FontFaces, nums: &HashMap<String, usize>| {
            word_wrap(&inlines_to_words(inlines, BODY_PT, fonts, nums), cell_w)
        };

        // Row height = tallest cell (by line count) × lh + top+bottom padding.
        let row_height = |wrapped: &[Vec<Vec<Word>>]| -> f32 {
            let max_lines = wrapped.iter().map(|l| l.len().max(1)).max().unwrap_or(1);
            max_lines as f32 * lh + v_pad * 2.0
        };

        self.y -= 2.0;

        // ── Header row ────────────────────────────────────────────────────────
        let header_wrapped: Vec<Vec<Vec<Word>>> = headers.iter()
            .map(|c| wrap_cell(c, &self.fonts, &self.footnote_nums))
            .collect();
        let h_row_h = row_height(&header_wrapped);
        self.ensure(h_row_h);
        let row_top = self.y;
        self.filled_rect(MARGIN_X, row_top - h_row_h, CONTENT_W, h_row_h, 0.88, 0.88, 0.92);
        self.y -= h_row_h;
        for (c, lines) in header_wrapped.iter().enumerate() {
            let x = MARGIN_X + c as f32 * col_w + h_pad;
            let mut cy = row_top - v_pad - lh;
            for line in lines {
                let mut lx = x;
                for (wi, word) in line.iter().enumerate() {
                    self.set_fill(0.08, 0.08, 0.08);
                    // Force bold for all header text
                    self.put_text_fb(&word.text, word.size, lx, cy, true, word.italic, word.mono);
                    lx += word.width();
                    if wi + 1 < line.len() { lx += word.spacing; }
                }
                cy -= lh;
            }
        }
        self.hline(MARGIN_X, MARGIN_X + CONTENT_W, self.y, 0.5, 0.5, 0.55, 0.5);

        // ── Data rows ─────────────────────────────────────────────────────────
        for row in rows {
            let row_wrapped: Vec<Vec<Vec<Word>>> = row.iter()
                .take(n_cols)
                .map(|c| wrap_cell(c, &self.fonts, &self.footnote_nums))
                .collect();
            let r_h = row_height(&row_wrapped);
            self.ensure(r_h);
            let row_top = self.y;
            self.y -= r_h;
            for (c, lines) in row_wrapped.iter().enumerate() {
                let x = MARGIN_X + c as f32 * col_w + h_pad;
                let mut cy = row_top - v_pad - lh;
                for line in lines {
                    let mut lx = x;
                    for (wi, word) in line.iter().enumerate() {
                        self.set_fill(word.color.0, word.color.1, word.color.2);
                        self.put_text_fb(&word.text, word.size, lx, cy, word.bold, word.italic, word.mono);
                        lx += word.width();
                        if wi + 1 < line.len() { lx += word.spacing; }
                    }
                    cy -= lh;
                }
            }
            self.hline(MARGIN_X, MARGIN_X + CONTENT_W, self.y, 0.80, 0.80, 0.80, 0.2);
        }

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


fn inlines_to_words(
    inlines:       &[Inline],
    default_size:  f32,
    fonts:         &FontFaces,
    footnote_nums: &HashMap<String, usize>,
) -> Vec<Word> {
    let mut out = Vec::new();
    // Use the actual advance width of a space in the body font for inter-word spacing.
    let spacing = fonts.space_w(default_size);

    for inline in inlines {
        // FootnoteRef: render as a small "[n]" marker in the text flow.
        if let Inline::FootnoteRef(label) = inline {
            let n = footnote_nums.get(label.as_str()).copied().unwrap_or(0);
            let text = format!("[{n}]");
            let size = default_size * 0.80;
            out.push(Word {
                width_mm: fonts.measure(&text, size, false, false, false),
                text,
                bold:   false,
                italic: false,
                mono:   false,
                color:  (0.17, 0.47, 0.64),
                size,
                spacing,
            });
            continue;
        }

        let (text, bold, italic, mono, color): (&str, bool, bool, bool, (f32, f32, f32)) = match inline {
            Inline::Text(s)       => (s, false, false, false, (0.10, 0.10, 0.10)),
            Inline::Bold(s)       => (s, true,  false, false, (0.05, 0.05, 0.05)),
            Inline::Italic(s)     => (s, false, true,  false, (0.15, 0.15, 0.15)),
            Inline::BoldItalic(s) => (s, true,  true,  false, (0.05, 0.05, 0.05)),
            Inline::Code(s)       => (s, false, false, true,  (0.12, 0.12, 0.12)),
            Inline::Link(t, _)    => (t, false, false, false, (0.17, 0.47, 0.64)),
            Inline::Image(_, a)   => (a, false, false, false, (0.50, 0.50, 0.50)),
            Inline::FootnoteRef(_) => unreachable!(),
        };
        let size = if mono { default_size * 0.92 } else { default_size };
        for raw in text.split_whitespace() {
            let width_mm = fonts.measure(raw, size, bold, italic, mono);
            out.push(Word { text: raw.to_string(), bold, italic, mono, color, size, width_mm, spacing });
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
        let space = if cur.is_empty() { 0.0 } else { word.spacing };
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

// ── PDF outline injection via lopdf ──────────────────────────────────────────

struct OutlineNode {
    _level:   u32,
    title:    String,
    page_num: usize,
    parent:   Option<usize>,
    children: Vec<usize>,
    prev:     Option<usize>,
    next:     Option<usize>,
}

fn build_outline_tree(headings: &[(u32, usize, String)]) -> Vec<OutlineNode> {
    let mut nodes: Vec<OutlineNode> = Vec::new();
    // stack of (level, node_index)
    let mut stack: Vec<(u32, usize)> = Vec::new();

    for (level, page_num, title) in headings {
        // Pop ancestors at same or deeper level.
        while stack.last().map_or(false, |&(l, _)| l >= *level) {
            stack.pop();
        }
        let parent = stack.last().map(|&(_, idx)| idx);

        // Find prev sibling = last child of parent (or last root node).
        let prev = match parent {
            Some(p) => nodes[p].children.last().copied(),
            None    => {
                // last root node
                let mut last_root = None;
                for (i, n) in nodes.iter().enumerate() {
                    if n.parent.is_none() { last_root = Some(i); }
                }
                last_root
            }
        };

        let idx = nodes.len();
        nodes.push(OutlineNode {
            _level: *level,
            title: title.clone(),
            page_num: *page_num,
            parent,
            children: Vec::new(),
            prev,
            next: None,
        });

        // Wire prev.next → this node.
        if let Some(p) = prev {
            nodes[p].next = Some(idx);
        }
        // Register as child of parent.
        if let Some(p) = parent {
            nodes[p].children.push(idx);
        }

        stack.push((*level, idx));
    }
    nodes
}

fn count_descendants(idx: usize, nodes: &[OutlineNode]) -> i64 {
    nodes[idx].children.iter()
        .map(|&ci| 1 + count_descendants(ci, nodes))
        .sum()
}

fn pdf_utf16(s: &str) -> lopdf::Object {
    let mut bytes = vec![0xFE, 0xFF]; // BOM
    for unit in s.encode_utf16() {
        bytes.push((unit >> 8) as u8);
        bytes.push(unit as u8);
    }
    lopdf::Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}

fn inject_outlines(
    bytes: Vec<u8>,
    headings: &[(u32, usize, String)],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use lopdf::{Dictionary, Object, ObjectId};

    if headings.is_empty() {
        return Ok(bytes);
    }

    let mut doc = lopdf::Document::load_from(std::io::Cursor::new(&bytes))?;

    // pages() returns BTreeMap<page_number_1based, ObjectId>
    let pages: std::collections::BTreeMap<u32, ObjectId> = doc.get_pages();

    let nodes = build_outline_tree(headings);
    if nodes.is_empty() {
        return Ok(bytes);
    }

    // Allocate object IDs: root + one per node.
    let root_id: ObjectId = doc.new_object_id();
    let node_ids: Vec<ObjectId> = (0..nodes.len()).map(|_| doc.new_object_id()).collect();

    // Helper: resolve 0-based page_num to a lopdf page ObjectId.
    let page_ref = |page_num: usize| -> Option<ObjectId> {
        let page_1based = (page_num + 1) as u32;
        pages.get(&page_1based).copied()
    };

    // Build each outline item dictionary.
    for (i, node) in nodes.iter().enumerate() {
        let parent_ref = match node.parent {
            Some(p) => Object::Reference(node_ids[p]),
            None    => Object::Reference(root_id),
        };

        let mut dict = Dictionary::new();
        dict.set("Title",  pdf_utf16(&node.title));
        dict.set("Parent", parent_ref);

        if let Some(page_oid) = page_ref(node.page_num) {
            dict.set("Dest", Object::Array(vec![
                Object::Reference(page_oid),
                Object::Name(b"XYZ".to_vec()),
                Object::Null,
                Object::Null,
                Object::Null,
            ]));
        }

        let desc = count_descendants(i, &nodes);
        dict.set("Count", Object::Integer(desc));

        if let Some(prev) = node.prev {
            dict.set("Prev", Object::Reference(node_ids[prev]));
        }
        if let Some(next) = node.next {
            dict.set("Next", Object::Reference(node_ids[next]));
        }
        if let Some(&first) = node.children.first() {
            dict.set("First", Object::Reference(node_ids[first]));
        }
        if let Some(&last) = node.children.last() {
            dict.set("Last", Object::Reference(node_ids[last]));
        }

        doc.objects.insert(node_ids[i], Object::Dictionary(dict));
    }

    // Collect root-level nodes.
    let root_nodes: Vec<usize> = nodes.iter().enumerate()
        .filter(|(_, n)| n.parent.is_none())
        .map(|(i, _)| i)
        .collect();

    let total_count = nodes.len() as i64;
    let first_root = root_nodes.first().map(|&i| node_ids[i]);
    let last_root  = root_nodes.last().map(|&i| node_ids[i]);

    // Build root /Outlines dictionary.
    let mut root_dict = Dictionary::new();
    root_dict.set("Type", Object::Name(b"Outlines".to_vec()));
    root_dict.set("Count", Object::Integer(total_count));
    if let Some(fr) = first_root {
        root_dict.set("First", Object::Reference(fr));
    }
    if let Some(lr) = last_root {
        root_dict.set("Last", Object::Reference(lr));
    }
    doc.objects.insert(root_id, Object::Dictionary(root_dict));

    // Wire catalog.
    let catalog = doc.catalog_mut()?;
    catalog.set("Outlines", Object::Reference(root_id));
    catalog.set("PageMode", Object::Name(b"UseOutlines".to_vec()));

    let mut out = Vec::new();
    doc.save_to(&mut out)?;
    Ok(out)
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn export_pdf(doc: &ParsedDoc, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let title = dest.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Document".to_string());

    let mut r = Renderer::new(&title)?;

    // Build label → display_number map from ParsedDoc.
    r.footnote_nums = doc.footnote_map.iter()
        .map(|(label, &(num, _, _))| (label.clone(), num))
        .collect();

    for block in &doc.blocks {
        r.render_block(block, 0.0);
    }

    let headings = r.bookmarks.clone();
    let bytes = r.doc.save_to_bytes()?;
    let bytes = inject_outlines(bytes, &headings)?;
    std::fs::write(dest, bytes)?;
    Ok(())
}
