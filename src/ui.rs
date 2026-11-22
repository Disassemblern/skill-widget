use crate::{engine::Skill, worker::{self, Update}};
use eframe::egui::{self, RichText};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

const COPIED_FOR: Duration = Duration::from_millis(1500);

pub fn run() -> Result<(), String> {
    let (queries, query_rx) = mpsc::channel();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Skills")
            .with_inner_size([320.0, 460.0])
            .with_min_inner_size([200.0, 140.0])
            .with_always_on_top(),
        ..Default::default()
    };
    eframe::run_native(
        "skill-widget",
        options,
        Box::new(move |cc| {
            #[cfg(windows)]
            crate::draft::spawn(queries.clone());
            let updates = worker::spawn(cc.egui_ctx.clone(), query_rx);
            Ok(Box::new(Widget { updates, queries, state: Update::default(), search: String::new(), copied: None, scroll_top: false }))
        }),
    )
    .map_err(|e| e.to_string())
}

struct Widget {
    updates: Receiver<Update>,
    queries: Sender<String>,
    state: Update,
    search: String,
    copied: Option<(String, Instant)>,
    scroll_top: bool,
}

fn truncate(s: &str, n: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= n { flat } else { flat.chars().take(n).collect::<String>() + "..." }
}

/// First sentence, capped at `n` characters.
fn short(desc: &str, n: usize) -> String {
    let end = desc.match_indices(['.', '!', '?']).map(|(i, _)| i + 1).find(|&i| desc[i..].starts_with(' ') || i == desc.len());
    truncate(&desc[..end.unwrap_or(desc.len())], n)
}

fn title(s: &Skill) -> String {
    let tags = [(s.manual, "  (manual)"), (s.source == "Project", "  (project)")];
    tags.iter().filter(|(on, _)| *on).fold(format!("/{}", s.invoke), |t, (_, tag)| t + tag)
}

fn hover(s: &Skill, score: Option<f32>) -> String {
    let plugin = match s.invoke.split_once(':') {
        Some((name, _)) if !s.plugin_description.is_empty() => format!("\n\nPlugin {name}: {}", s.plugin_description),
        _ => String::new(),
    };
    let score = score.map(|v| format!("score {v:.3} - ")).unwrap_or_default();
    format!("{}\n\n{}{plugin}\n\n{score}click to copy", s.invoke, s.description)
}

impl eframe::App for Widget {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        for u in self.updates.try_iter() {
            if u.prompt != self.state.prompt {
                self.search.clear(); // a new Claude message takes over from the search box
            }
            self.state = u;
            self.scroll_top = true;
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.label(RichText::new(&self.state.status).small().weak());
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("type here, or just type in Claude")
                    .desired_width(f32::INFINITY),
            );
            if edit.changed() {
                let _ = self.queries.send(self.search.clone());
            }
            if self.search.is_empty() && !self.state.query.is_empty() {
                ui.label(RichText::new(format!("for: {}", truncate(&self.state.query, 90))).small().italics().weak());
            }
            let fresh = self.copied.as_ref().filter(|(_, at)| at.elapsed() < COPIED_FOR).map(|(n, _)| n.clone());
            if let Some(name) = fresh {
                ui.label(RichText::new(format!("copied /{name}, paste it into Claude")).small().strong());
                ui.ctx().request_repaint_after(COPIED_FOR);
            }
            ui.separator();

            let scores: Vec<f32> = self.state.rows.iter().filter_map(|r| r.score).collect();
            let hi = scores.first().copied().unwrap_or(1.0);
            let lo = scores.last().copied().unwrap_or(0.0);
            let mut area = egui::ScrollArea::vertical();
            if std::mem::take(&mut self.scroll_top) {
                area = area.vertical_scroll_offset(0.0); // new ranking: show the best matches
            }
            area.show(ui, |ui| {
                for row in &self.state.rows {
                    let s = &row.skill;
                    let resp = ui
                        .horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 6.0), egui::Sense::hover());
                            if let Some(score) = row.score {
                                let rel = if hi > lo { (score - lo) / (hi - lo) } else { 1.0 };
                                ui.painter().rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
                                let mut bar = rect;
                                bar.set_width(rect.width() * rel.max(0.08));
                                ui.painter().rect_filled(bar, 2.0, ui.visuals().selection.bg_fill);
                            }
                            ui.vertical(|ui| {
                                let name = ui.add(egui::Label::new(RichText::new(title(s)).strong()).sense(egui::Sense::click()).truncate());
                                let about = ui.add(egui::Label::new(RichText::new(short(&s.description, 100)).small().weak()).sense(egui::Sense::click()).wrap());
                                name.union(about)
                            })
                            .inner
                        })
                        .inner
                        .on_hover_text(hover(s, row.score));
                    if resp.clicked() {
                        ui.ctx().copy_text(format!("/{}", s.invoke));
                        self.copied = Some((s.invoke.clone(), Instant::now()));
                    }
                    ui.add_space(4.0);
                }
            });
        });
    }
}
