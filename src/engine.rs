use crate::{config::Config, discover, embed::{DocCache, Embedder}, rank};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct Skill {
    /// What you type after "/", e.g. "superpowers:brainstorming".
    pub invoke: String,
    pub description: String,
    /// disable-model-invocation: only you can call it, Claude won't.
    pub manual: bool,
    /// "Personal", "Project", "Claude.ai" or the plugin name.
    pub source: String,
    pub plugin_description: String,
    /// Injected into every session by its plugin's SessionStart hook, so never worth suggesting.
    pub always_on: bool,
}

/// Every skill worth suggesting for a session in `cwd`.
pub fn all_skills(cwd: &str, hide: &[String]) -> Vec<Skill> {
    usable(discover::global().into_iter().chain(discover::project(cwd)), hide)
}

/// Drop always-on and hidden skills and duplicate names (first one wins).
pub fn usable(skills: impl IntoIterator<Item = Skill>, hide: &[String]) -> Vec<Skill> {
    let mut seen = HashSet::new();
    skills.into_iter().filter(|s| !s.always_on && !hide.contains(&s.invoke) && seen.insert(s.invoke.clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sk(invoke: &str, always_on: bool) -> Skill {
        Skill { invoke: invoke.into(), description: String::new(), manual: false, source: String::new(), plugin_description: String::new(), always_on }
    }

    #[test]
    fn usable_drops_always_on_hidden_and_duplicates() {
        let all = vec![sk("a", false), sk("ponytail:ponytail", true), sk("b", false), sk("a", false), sk("c", false)];
        let names: Vec<String> = usable(all, &["b".to_string()]).into_iter().map(|s| s.invoke).collect();
        assert_eq!(names, vec!["a", "c"]);
    }
}

pub struct Engine {
    pub cfg: Config,
    pub skills: Vec<Skill>,
    embedder: Embedder,
    cache: DocCache,
    vecs: Vec<Vec<f32>>,
}

impl Engine {
    pub fn new(cfg: Config, skills: Vec<Skill>) -> Result<Self, String> {
        let embedder = Embedder::new(&cfg)?;
        let cache = DocCache::open(&cfg);
        let mut engine = Self { cfg, skills: Vec::new(), embedder, cache, vecs: Vec::new() };
        engine.set_skills(skills)?;
        Ok(engine)
    }

    /// Swap the skill set without reloading the model; only unseen descriptions get embedded.
    pub fn set_skills(&mut self, skills: Vec<Skill>) -> Result<(), String> {
        if skills == self.skills && !skills.is_empty() {
            return Ok(());
        }
        let docs: Vec<String> = skills.iter().map(|s| format!("{}{}: {}", self.cfg.doc_prefix, s.invoke, s.description)).collect();
        self.vecs = self.cache.embed(&mut self.embedder, &docs)?;
        self.skills = skills;
        Ok(())
    }

    pub fn rank(&mut self, query: &str) -> Result<Vec<(usize, f32)>, String> {
        let q = self.embedder.embed(&[format!("{}{}", self.cfg.query_prefix, query)])?;
        let q = q.first().ok_or("model returned no embedding")?;
        Ok(rank::rank(q, &self.vecs))
    }
}
