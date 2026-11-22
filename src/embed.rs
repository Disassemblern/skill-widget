use crate::config::{self, Config};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf};

pub enum Embedder {
    Fast(TextEmbedding),
    Http { url: String, model: String, key: Option<String> },
}

impl Embedder {
    pub fn new(cfg: &Config) -> Result<Self, String> {
        match cfg.backend.as_str() {
            "fastembed" => {
                let model: EmbeddingModel = cfg.model.parse()?;
                let opts = TextInitOptions::new(model)
                    .with_cache_dir(config::dir().join("models"))
                    .with_show_download_progress(false);
                Ok(Self::Fast(TextEmbedding::try_new(opts).map_err(|e| e.to_string())?))
            }
            "http" => Ok(Self::Http {
                url: cfg.url.clone(),
                model: cfg.model.clone(),
                key: Some(&cfg.api_key_env).filter(|k| !k.is_empty()).and_then(|k| std::env::var(k).ok()),
            }),
            other => Err(format!("unknown backend '{other}', use \"fastembed\" or \"http\"")),
        }
    }

    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        match self {
            Self::Fast(m) => m.embed(texts, None).map_err(|e| e.to_string()),
            Self::Http { url, model, key } => http_embed(url, model, key.as_deref(), texts),
        }
    }
}

fn http_embed(url: &str, model: &str, key: Option<&str>, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    #[derive(Deserialize)]
    struct Item {
        embedding: Vec<f32>,
        #[serde(default)]
        index: usize,
    }
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<Item>,
    }
    let req = ureq::post(url);
    let req = match key {
        Some(k) => req.header("Authorization", format!("Bearer {k}")),
        None => req,
    };
    let mut resp: Resp = req
        .send_json(serde_json::json!({ "model": model, "input": texts }))
        .map_err(|e| format!("{url}: {e}"))?
        .body_mut()
        .read_json()
        .map_err(|e| format!("{url}: {e}"))?;
    resp.data.sort_by_key(|i| i.index);
    Ok(resp.data.into_iter().map(|i| i.embedding).collect())
}

/// Skill vectors saved per model, so restarts and switching back to a model are instant.
pub struct DocCache {
    path: PathBuf,
    map: HashMap<String, Vec<f32>>,
}

impl DocCache {
    pub fn open(cfg: &Config) -> Self {
        let slug: String = format!("{}-{}", cfg.backend, cfg.model)
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
            .collect();
        let path = config::dir().join("cache").join(format!("{slug}.json"));
        let map = fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Self { path, map }
    }

    pub fn embed(&mut self, embedder: &mut Embedder, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let missing: Vec<String> = texts.iter().filter(|t| !self.map.contains_key(*t)).cloned().collect();
        if !missing.is_empty() {
            let vecs = embedder.embed(&missing)?;
            self.map.extend(missing.into_iter().zip(vecs));
            if let Some(dir) = self.path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&self.path, serde_json::to_string(&self.map).unwrap_or_default());
        }
        Ok(texts.iter().map(|t| self.map[t].clone()).collect())
    }
}
