use crate::{config, engine::{self, Engine, Skill}};
use eframe::egui;
use std::{
    fs,
    path::Path,
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::{Duration, SystemTime},
};

pub struct Row {
    pub skill: Skill,
    /// None when there is nothing to rank against yet.
    pub score: Option<f32>,
}

#[derive(Default)]
pub struct Update {
    pub status: String,
    pub prompt: String,
    /// What the rows are ranked against: the live draft, the search box, or the last sent prompt.
    pub query: String,
    pub rows: Vec<Row>,
}

fn mtime(p: &Path) -> Option<SystemTime> {
    fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// (prompt, cwd) from prompt.json, written by `skill-widget hook` on every sent message.
fn read_prompt(p: &Path) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(p).ok()?).ok()?;
    let field = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    Some((field("prompt"), field("cwd")))
}

/// Background loop: watches config.toml and prompt.json (300ms poll) plus the search box and
/// live draft, and sends a fresh ranking whenever any of them changes. Skills are rediscovered
/// on every sent message, so new installs show up without a restart.
pub fn spawn(ctx: egui::Context, queries: Receiver<String>) -> Receiver<Update> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let send = |u: Update| {
            let _ = tx.send(u);
            ctx.request_repaint();
        };
        let (cfg_path, prompt_path) = (config::path(), config::dir().join("prompt.json"));
        let mut engine: Option<Engine> = None;
        let (mut status, mut prompt, mut cwd, mut search) = (String::new(), String::new(), String::new(), String::new());
        if let Some((p, c)) = read_prompt(&prompt_path) {
            (prompt, cwd) = (p, c);
        }
        let (mut seen_cfg, mut seen_prompt) = (None, mtime(&prompt_path));
        let (mut first, mut dirty) = (true, false);
        loop {
            if first || mtime(&cfg_path) != seen_cfg {
                first = false;
                send(Update { status: "loading model (first run downloads it)...".into(), prompt: prompt.clone(), ..Default::default() });
                let built = config::load().and_then(|cfg| {
                    let label = format!("{} / {}", cfg.backend, cfg.model);
                    let skills = engine::all_skills(&cwd, &cfg.hide);
                    Engine::new(cfg, skills).map(|e| (e, label))
                });
                seen_cfg = mtime(&cfg_path);
                match built {
                    Ok((e, label)) => {
                        status = label;
                        engine = Some(e);
                    }
                    Err(err) => status = err, // keep the previous engine, if any
                }
                dirty = true;
            }

            let pt = mtime(&prompt_path);
            if pt != seen_prompt {
                seen_prompt = pt;
                if let Some((p, c)) = read_prompt(&prompt_path) {
                    (prompt, cwd) = (p, c);
                    search.clear();
                    dirty = true;
                    if let Some(e) = engine.as_mut() {
                        let skills = engine::all_skills(&cwd, &e.cfg.hide);
                        if let Err(err) = e.set_skills(skills) {
                            status = err;
                        }
                    }
                }
            }

            if dirty {
                dirty = false;
                let query = if search.trim().is_empty() { &prompt } else { &search };
                let mut shown = match &engine {
                    Some(e) => format!("{status} - {} skills", e.skills.len()),
                    None => status.clone(),
                };
                // Always every skill: ranked when there is a query, otherwise in list order.
                let rows = match engine.as_mut() {
                    Some(e) => {
                        let ranked = if query.trim().is_empty() { Ok(Vec::new()) } else { e.rank(query) };
                        match ranked {
                            Ok(r) if !r.is_empty() => r.into_iter().map(|(i, score)| Row { skill: e.skills[i].clone(), score: Some(score) }).collect(),
                            other => {
                                if let Err(err) = other {
                                    shown = err;
                                }
                                e.skills.iter().map(|s| Row { skill: s.clone(), score: None }).collect()
                            }
                        }
                    }
                    None => Vec::new(),
                };
                send(Update { status: shown, prompt: prompt.clone(), query: query.clone(), rows });
            }

            match queries.recv_timeout(Duration::from_millis(300)) {
                Ok(q) => {
                    search = queries.try_iter().last().unwrap_or(q);
                    dirty = true;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    });
    rx
}
