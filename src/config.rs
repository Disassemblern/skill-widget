use serde::Deserialize;
use std::{fs, path::PathBuf};

/// Everything the widget reads and writes lives here: ~/.claude/skill-widget
pub fn dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    home.map(PathBuf::from).unwrap_or_default().join(".claude").join("skill-widget")
}

pub fn path() -> PathBuf {
    dir().join("config.toml")
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub backend: String,
    pub model: String,
    pub query_prefix: String,
    pub doc_prefix: String,
    pub url: String,
    pub api_key_env: String,
    pub hide: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backend: "fastembed".into(),
            model: "BGESmallENV15".into(),
            query_prefix: String::new(),
            doc_prefix: String::new(),
            url: "http://localhost:11434/v1/embeddings".into(),
            api_key_env: String::new(),
            hide: Vec::new(),
        }
    }
}

const DEFAULT_TOML: &str = r#"# skill-widget config. Save this file and the running widget reloads it.
# Compare models with numbers: `skill-widget eval --model <name>` (run from the project folder).

# "fastembed" = local model, downloaded once into ~/.claude/skill-widget/models
#               list the built-in names with: skill-widget models
# "http"      = any OpenAI-compatible /v1/embeddings server (Ollama, LM Studio, llama.cpp, OpenAI...)
backend = "fastembed"
model = "BGESmallENV15"

# Some models expect prefixes, check the model card. Examples:
#   e5:     query_prefix = "query: "         doc_prefix = "passage: "
#   nomic:  query_prefix = "search_query: "  doc_prefix = "search_document: "
query_prefix = ""
doc_prefix = ""

# Used only when backend = "http":
url = "http://localhost:11434/v1/embeddings"
# Name of an environment variable holding the API key (never put the key itself here).
api_key_env = ""

# Skills never to show, e.g. ["ponytail:ponytail-help"]. Skills a plugin injects into
# every session (like ponytail:ponytail) are already hidden automatically.
hide = []
"#;

pub fn load() -> Result<Config, String> {
    let p = path();
    if !p.exists() {
        fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
        fs::write(&p, DEFAULT_TOML).map_err(|e| e.to_string())?;
    }
    let text = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    toml::from_str(&text).map_err(|e| format!("config.toml: {e}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_file_matches_defaults() {
        let parsed: super::Config = toml::from_str(super::DEFAULT_TOML).unwrap();
        assert_eq!(parsed, super::Config::default());
    }
}
