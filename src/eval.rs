use crate::{config, engine::{self, Engine}};
use serde::Deserialize;
use std::{fs, time::Instant};

#[derive(Deserialize)]
struct Case {
    prompt: String,
    expect: String,
}

#[derive(Deserialize)]
struct EvalFile {
    case: Vec<Case>,
}

/// `skill-widget eval [--model NAME] [--file eval.toml]`: how often the expected skill ranks first / top 3.
pub fn run(args: &[String]) -> Result<(), String> {
    let flag = |name: &str| args.iter().position(|a| a == name).map(|i| args.get(i + 1).cloned().ok_or(format!("{name} needs a value")));
    let mut cfg = config::load()?;
    if let Some(model) = flag("--model").transpose()? {
        cfg.model = model;
    }
    let file = flag("--file").transpose()?.unwrap_or_else(|| "eval.toml".into());
    let cases = toml::from_str::<EvalFile>(&fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?)
        .map_err(|e| format!("{file}: {e}"))?
        .case;

    let started = Instant::now();
    let cwd = std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    let skills = engine::all_skills(&cwd, &cfg.hide);
    let mut engine = Engine::new(cfg.clone(), skills)?;
    println!("{} / {}: {} skills ready in {:.1}s\n", cfg.backend, cfg.model, engine.skills.len(), started.elapsed().as_secs_f32());

    let (mut n, mut top1, mut top3, mut mrr, mut ms) = (0, 0, 0, 0.0, 0.0);
    for case in &cases {
        if !engine.skills.iter().any(|s| s.invoke == case.expect) {
            println!("skip  {} (not installed)", case.expect);
            continue;
        }
        let t = Instant::now();
        let ranked = engine.rank(&case.prompt)?;
        ms += t.elapsed().as_secs_f64() * 1000.0;
        let pos = ranked.iter().position(|(i, _)| engine.skills[*i].invoke == case.expect).unwrap_or(ranked.len());
        n += 1;
        top1 += usize::from(pos == 0);
        top3 += usize::from(pos < 3);
        mrr += 1.0 / (pos as f64 + 1.0);
        let got = &engine.skills[ranked[0].0].invoke;
        let mark = if pos == 0 { "ok  " } else if pos < 3 { "top3" } else { "MISS" };
        println!("{mark}  #{:<2} {:<40} got {got}", pos + 1, case.expect);
    }
    if n == 0 {
        return Err("no runnable cases".into());
    }
    let pct = |k: usize| 100.0 * k as f64 / n as f64;
    println!("\ntop-1 {:.0}%   top-3 {:.0}%   MRR {:.3}   {:.0} ms/query   ({n} cases)", pct(top1), pct(top3), mrr / n as f64, ms / n as f64);
    Ok(())
}
