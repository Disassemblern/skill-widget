//! `hook`: the UserPromptSubmit hook body. `install` / `uninstall`: register it in ~/.claude/settings.json.
use crate::{config, discover};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Save the sent prompt and the session's cwd for the widget. Prints nothing (hook stdout would be
/// added to Claude's context) and never fails, so a problem here can never block a prompt.
pub fn hook() {
    let mut raw = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut raw);
    let Ok(event) = serde_json::from_slice::<Value>(&raw) else { return }; // keep the last good prompt
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    let out = json!({ "prompt": event["prompt"].as_str().unwrap_or(""), "cwd": event["cwd"].as_str().unwrap_or(""), "ts": ts });
    let dir = config::dir();
    let tmp = dir.join("prompt.tmp");
    let _ = fs::create_dir_all(&dir).and_then(|_| fs::write(&tmp, out.to_string())).and_then(|_| fs::rename(&tmp, dir.join("prompt.json")));
}

fn settings_path() -> PathBuf {
    discover::home().join(".claude").join("settings.json")
}

fn is_ours(hook: &Value) -> bool {
    hook["command"].as_str().is_some_and(|c| c.contains("skill-widget") && c.trim_end().ends_with(" hook"))
}

fn read_settings(path: &Path) -> Result<Value, String> {
    match fs::read_to_string(path) {
        Err(_) => Ok(json!({})),
        Ok(t) => serde_json::from_str(t.trim_start_matches('\u{feff}')).map_err(|e| format!("{}: {e}. Not touching it.", path.display())),
    }
}

fn save(path: &Path, settings: &Value) -> Result<(), String> {
    if path.exists() {
        fs::copy(path, path.with_extension("json.bak")).map_err(|e| e.to_string())?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, text + "\n").map_err(|e| e.to_string())
}

/// Remove our hook entries, and any groups or lists left empty by that.
fn remove_ours(settings: &mut Value) {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else { return };
    if let Some(groups) = hooks.get_mut("UserPromptSubmit").and_then(Value::as_array_mut) {
        for g in groups.iter_mut() {
            if let Some(list) = g.get_mut("hooks").and_then(Value::as_array_mut) {
                list.retain(|h| !is_ours(h));
            }
        }
        groups.retain(|g| g["hooks"].as_array().is_none_or(|l| !l.is_empty()));
        if groups.is_empty() {
            hooks.remove("UserPromptSubmit");
        }
    }
}

fn add_ours(settings: &mut Value, command: &str) -> Result<(), String> {
    let root = settings.as_object_mut().ok_or("settings.json is not a JSON object")?;
    let hooks = root.entry("hooks").or_insert_with(|| json!({})).as_object_mut().ok_or("\"hooks\" is not an object")?;
    let list = hooks.entry("UserPromptSubmit").or_insert_with(|| json!([])).as_array_mut().ok_or("\"UserPromptSubmit\" is not a list")?;
    list.push(json!({ "hooks": [{ "type": "command", "command": command, "timeout": 5 }] }));
    Ok(())
}

pub fn install() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // Forward slashes work in both cmd and bash, which Claude Code may use to run hooks.
    let command = format!("\"{}\" hook", exe.display().to_string().replace('\\', "/"));
    let path = settings_path();
    let mut settings = read_settings(&path)?;
    remove_ours(&mut settings);
    add_ours(&mut settings, &command)?;
    save(&path, &settings)?;
    println!("Added a UserPromptSubmit hook to {}:\n  {command}\nBackup: settings.json.bak. Restart open Claude Code sessions to pick it up.", path.display());
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let path = settings_path();
    let mut settings = read_settings(&path)?;
    remove_ours(&mut settings);
    save(&path, &settings)?;
    println!("Removed the skill-widget hook from {}.", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_is_idempotent_and_uninstall_keeps_other_hooks() {
        let other = json!({ "hooks": [{ "type": "command", "command": "python other.py" }] });
        let mut s = json!({ "model": "x", "hooks": { "UserPromptSubmit": [other.clone()] } });
        add_ours(&mut s, "\"C:/a/skill-widget.exe\" hook").unwrap();
        remove_ours(&mut s);
        add_ours(&mut s, "\"C:/b/skill-widget.exe\" hook").unwrap();
        assert_eq!(s["hooks"]["UserPromptSubmit"].as_array().unwrap().len(), 2);
        remove_ours(&mut s);
        assert_eq!(s, json!({ "model": "x", "hooks": { "UserPromptSubmit": [other] } }));
    }

    #[test]
    fn uninstall_drops_empty_list() {
        let mut s = json!({ "hooks": {} });
        add_ours(&mut s, "\"/usr/bin/skill-widget\" hook").unwrap();
        remove_ours(&mut s);
        assert_eq!(s, json!({ "hooks": {} }));
    }
}
