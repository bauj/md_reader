use egui::{Color32, Frame, FontId, RichText, TextFormat, Ui};
use egui::text::LayoutJob;
use egui_extras::{Column, TableBuilder};
use crate::markdown::{Block, Inline, ParsedDoc};
use crate::markdown::highlight::Highlighter;

pub fn render_markdown(
    ui:        &mut Ui,
    doc:       &ParsedDoc,
    scroll_to: Option<usize>,
    hl:        &mut Highlighter,
) {
    for (i, block) in doc.blocks.iter().enumerate() {
        ui.push_id(i, |ui| {
            render_block(ui, block, hl);
            if scroll_to == Some(i) {
                ui.scroll_to_cursor(Some(egui::Align::TOP));
            }
        });
        ui.add_space(4.0);
    }
}

fn render_block(ui: &mut Ui, block: &Block, hl: &mut Highlighter) {
    match block {
        Block::Heading(level, inlines) => {
            let size = match level {
                1 => 32.0,
                2 => 26.0,
                3 => 22.0,
                4 => 18.0,
                5 => 16.0,
                _ => 14.0,
            };
            ui.horizontal_wrapped(|ui| {
                for inline in inlines {
                    render_inline(ui, inline, Some(size), true);
                }
            });
            if *level <= 2 {
                ui.separator();
            }
        }

        Block::Paragraph(inlines) => {
            ui.horizontal_wrapped(|ui| {
                for inline in inlines {
                    render_inline(ui, inline, None, false);
                }
            });
        }

        Block::CodeBlock(lang, code) => {
            let lines = hl.highlight(lang, code);

            // Build a LayoutJob so mixed-color spans render as a single label,
            // preserving newlines and indentation faithfully.
            let mut job = LayoutJob::default();
            for line_spans in lines {
                for (color, text) in line_spans {
                    job.append(
                        text,
                        0.0,
                        TextFormat {
                            font_id: FontId::monospace(13.0),
                            color:   *color,
                            ..Default::default()
                        },
                    );
                }
            }

            Frame::canvas(&ui.style())
                .fill(Color32::from_gray(28))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if !lang.is_empty() {
                        ui.label(
                            RichText::new(lang.as_str())
                                .size(10.0)
                                .color(Color32::from_gray(140)),
                        );
                        ui.add_space(2.0);
                    }
                    ui.label(job);
                });
        }

        Block::BlockQuote(inlines) => {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(3.0, ui.spacing().interact_size.y.max(16.0)),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 2.0, Color32::from_gray(100));
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        for inline in inlines {
                            render_inline(ui, inline, None, false);
                        }
                    });
                });
            });
        }

        Block::List(ordered, items) => {
            for (i, item_inlines) in items.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    let bullet = if *ordered {
                        format!("{}.", i + 1)
                    } else {
                        "•".to_string()
                    };
                    ui.label(RichText::new(bullet).size(14.0));
                    ui.add_space(4.0);
                    for inline in item_inlines {
                        render_inline(ui, inline, None, false);
                    }
                });
            }
        }

        Block::Table(headers, rows) => {
            let col_count = headers.len().max(1);

            Frame::canvas(&ui.style())
                .fill(Color32::from_gray(24))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    TableBuilder::new(ui)
                        .striped(true)
                        .columns(Column::remainder().resizable(true), col_count)
                        .header(24.0, |mut row| {
                            for header in headers {
                                row.col(|ui| {
                                    ui.label(RichText::new(header.as_str()).strong());
                                });
                            }
                        })
                        .body(|mut body| {
                            for data_row in rows {
                                body.row(20.0, |mut row| {
                                    for cell in data_row.iter() {
                                        row.col(|ui| {
                                            ui.label(cell.as_str());
                                        });
                                    }
                                    for _ in data_row.len()..col_count {
                                        row.col(|_| {});
                                    }
                                });
                            }
                        });
                });
        }

        Block::Rule => {
            ui.separator();
        }
    }
}

fn render_inline(ui: &mut Ui, inline: &Inline, size: Option<f32>, strong: bool) {
    let base = size.unwrap_or(14.0);
    match inline {
        Inline::Text(t) => {
            let rt = RichText::new(t.as_str()).size(base);
            ui.label(if strong { rt.strong() } else { rt });
        }
        Inline::Bold(t) => {
            ui.label(RichText::new(t.as_str()).size(base).strong());
        }
        Inline::Italic(t) => {
            ui.label(RichText::new(t.as_str()).size(base).italics());
        }
        Inline::BoldItalic(t) => {
            ui.label(RichText::new(t.as_str()).size(base).strong().italics());
        }
        Inline::Code(c) => {
            Frame::canvas(&ui.style())
                .fill(Color32::from_gray(45))
                .inner_margin(egui::Margin::symmetric(3, 1))
                .corner_radius(3.0)
                .show(ui, |ui| {
                    ui.monospace(c.as_str());
                });
        }
        Inline::Link(text, url) => {
            if ui.link(text.as_str()).clicked() {
                let _ = open::that(url.as_str());
            }
        }
    }
}
