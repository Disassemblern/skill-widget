//! Finds every skill Claude Code can call: personal, enabled plugins, claude.ai-synced and project.
use crate::engine::Skill;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default()
}

/// Personal skills and commands, enabled plugins, and claude.ai-synced skills.
pub fn global() -> Vec<Skill> {
    let claude = home().join(".claude");
    let personal = skills_in(&claude.join("skills"), "", "Personal").into_iter().chain(commands_in(&claude.join("commands"), "", "Personal"));
    let synced = newest_dir(&claude.join("skills").join("synced")).map(|d| skills_in(&d, "anthropic-skills:", "Claude.ai")).unwrap_or_default();
    personal.chain(plugins(&claude)).chain(synced).collect()
}

/// Skills and commands in `<dir>/.claude/` for the session's cwd and its parents,
/// stopping before the home folder (already covered by `global`).
pub fn project(cwd: &str) -> Vec<Skill> {
    if cwd.is_empty() {
        return Vec::new();
    }
    let home = home();
    Path::new(cwd)
        .ancestors()
        .take_while(|d| *d != home.as_path())
        .flat_map(|d| {
            let base = d.join(".claude");
            skills_in(&base.join("skills"), "", "Project").into_iter().chain(commands_in(&base.join("commands"), "", "Project"))
        })
        .collect()
}

fn read_json(p: &Path) -> Value {
    fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()).unwrap_or(Value::Null)
}

fn plugins(claude: &Path) -> Vec<Skill> {
    let settings = read_json(&claude.join("settings.json"));
    let installed = read_json(&claude.join("plugins").join("installed_plugins.json"));
    let Some(map) = installed["plugins"].as_object() else { return Vec::new() };
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    keys.into_iter()
        .filter(|k| settings["enabledPlugins"][k.as_str()].as_bool() == Some(true))
        .filter_map(|k| {
            let root = PathBuf::from(map[k][0]["installPath"].as_str()?);
            Some(plugin_skills(k.split('@').next().unwrap_or(k), &root))
        })
        .flatten()
        .collect()
}

fn plugin_skills(name: &str, root: &Path) -> Vec<Skill> {
    let meta = read_json(&root.join(".claude-plugin").join("plugin.json"));
    let about = meta["description"].as_str().unwrap_or_default().to_string();
    let hooks_file = match meta["hooks"].as_str() {
        Some(rel) => root.join(rel),
        None => root.join("hooks").join("hooks.json"),
    };
    let scripts = session_start_scripts(&hooks_file);
    let prefix = format!("{name}:");
    skills_in(&root.join("skills"), &prefix, name)
        .into_iter()
        .chain(commands_in(&root.join("commands"), &prefix, name))
        .map(|s| {
            let always_on = injects(&scripts, &s.invoke[prefix.len()..]);
            Skill { always_on, plugin_description: about.clone(), ..s }
        })
        .collect()
}

/// Text of every file next to the plugin's hooks file, if it registers a SessionStart hook.
fn session_start_scripts(hooks_file: &Path) -> String {
    let registers = fs::read_to_string(hooks_file).is_ok_and(|t| t.contains("SessionStart"));
    let Some(dir) = hooks_file.parent().filter(|_| registers) else { return String::new() };
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() < 200_000))
        .filter_map(|p| fs::read(p).ok())
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// True when hook scripts load `skills/<name>/...` or `path.join(..., 'skills', '<name>', ...)`:
/// that skill is injected into every session, so suggesting it is pointless.
fn injects(scripts: &str, name: &str) -> bool {
    const SEP: &[char] = &['\\', '/', '\'', '"', ',', ' '];
    scripts.match_indices(name).any(|(i, _)| {
        let before = &scripts[..i];
        let trimmed = before.trim_end_matches(SEP);
        let gap = before.len() - trimmed.len();
        scripts[i + name.len()..].starts_with(['\\', '/', '\'', '"']) && (1..=6).contains(&gap) && trimmed.ends_with("skills")
    })
}

fn newest_dir(p: &Path) -> Option<PathBuf> {
    fs::read_dir(p)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

fn sorted_entries(p: &Path, keep: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(p).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| keep(p)).collect();
    v.sort();
    v
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn skill(invoke: String, description: String, manual: bool, source: &str) -> Skill {
    Skill { invoke, description, manual, source: source.into(), plugin_description: String::new(), always_on: false }
}

fn skills_in(dir: &Path, prefix: &str, source: &str) -> Vec<Skill> {
    sorted_entries(dir, |p| p.join("SKILL.md").is_file())
        .into_iter()
        .filter_map(|d| {
            let (name, description, manual) = parse(&fs::read_to_string(d.join("SKILL.md")).ok()?);
            Some(skill(format!("{prefix}{}", name.unwrap_or_else(|| file_name(&d))), description, manual, source))
        })
        .collect()
}

fn commands_in(dir: &Path, prefix: &str, source: &str) -> Vec<Skill> {
    sorted_entries(dir, |p| p.extension().is_some_and(|x| x == "md"))
        .into_iter()
        .filter_map(|f| {
            let (_, description, _) = parse(&fs::read_to_string(&f).ok()?);
            Some(skill(format!("{prefix}{}", file_name(&f).trim_end_matches(".md")), description, true, source))
        })
        .collect()
}

/// Frontmatter of a SKILL.md or command file: (name, description, manual-only).
fn parse(text: &str) -> (Option<String>, String, bool) {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let (block, body) = match text.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---")) {
        Some((block, body)) => (block, body),
        None => ("", text.as_str()),
    };
    let clean = |s: &str| s.trim().trim_matches(|c| " >-|\"'".contains(c)).to_string();
    let (mut name, mut desc, mut manual, mut in_desc) = (None, String::new(), false, false);
    for line in block.lines() {
        let is_key = line.chars().next().is_some_and(|c| c.is_alphanumeric()) && line.contains(':');
        if is_key {
            in_desc = false;
            let (key, value) = line.split_once(':').unwrap_or_default();
            match key.trim() {
                "name" => name = Some(clean(value)),
                "description" => {
                    desc = value.to_string();
                    in_desc = true;
                }
                "disable-model-invocation" => manual = value.trim() == "true",
                _ => {}
            }
        } else if in_desc {
            desc = format!("{desc} {line}");
        }
    }
    let desc = clean(&desc.split_whitespace().collect::<Vec<_>>().join(" "));
    let desc = if desc.is_empty() {
        body.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')).unwrap_or("").to_string()
    } else {
        desc
    };
    (name.filter(|n| !n.is_empty()), desc, manual)
}

#[cfg(test)]
mod tests {
    use super::{injects, parse};

    #[test]
    fn folded_description_and_flags() {
        let md = "---\r\nname: my-skill\r\ndescription: >\r\n  Does one thing.\r\n  Then another.\r\ndisable-model-invocation: true\r\n---\r\n# Body\r\n";
        assert_eq!(parse(md), (Some("my-skill".into()), "Does one thing. Then another.".into(), true));
    }

    #[test]
    fn falls_back_to_body() {
        assert_eq!(parse("# Title\n\nFirst real line.\n"), (None, "First real line.".into(), false));
    }

    #[test]
    fn detects_injected_skills() {
        assert!(injects(r#"cat "$ROOT/skills/using-superpowers/SKILL.md""#, "using-superpowers"));
        assert!(injects("path.join(__dirname, '..', 'skills', 'ponytail', 'SKILL.md')", "ponytail"));
        assert!(!injects("path.join(__dirname, 'skills', 'ponytail-audit', 'SKILL.md')", "ponytail"));
        assert!(!injects("the ponytail mode is active", "ponytail"));
    }
}
