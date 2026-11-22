//! Live draft: reads the text of Claude's chat box while you type, via Windows UI Automation
//! (the screen-reader interface). Only the focused element is checked, and only when it is
//! Claude's "Message input" box; nothing else is read, and nothing is stored.
use std::{sync::mpsc::Sender, thread, time::Duration};
use uiautomation::{UIAutomation, patterns::UIValuePattern};

const POLL: Duration = Duration::from_millis(400);

/// Claude Code's chat box in the VS Code extension, as UI Automation reports it.
fn is_claude_input(name: &str, class: &str) -> bool {
    name == "Message input" && class.starts_with("messageInput")
}

/// Sends the draft to `queries` whenever it changes.
pub fn spawn(queries: Sender<String>) {
    thread::spawn(move || {
        let Ok(automation) = UIAutomation::new() else { return };
        let mut last = String::new();
        loop {
            thread::sleep(POLL);
            let Some(text) = read(&automation) else { continue };
            if text != last {
                last = text.clone();
                if queries.send(text).is_err() {
                    return; // widget closed
                }
            }
        }
    });
}

fn read(automation: &UIAutomation) -> Option<String> {
    let el = automation.get_focused_element().ok()?;
    if !is_claude_input(&el.get_name().ok()?, &el.get_classname().ok()?) {
        return None;
    }
    let value = el.get_pattern::<UIValuePattern>().ok()?.get_value().ok()?;
    let value = value.trim();
    // An empty box reports its placeholder ("Queue another message…") as the value.
    (!value.is_empty() && !value.ends_with('\u{2026}')).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_only_claude_box() {
        assert!(super::is_claude_input("Message input", "messageInput_cKsPxg"));
        assert!(!super::is_claude_input("Message input", "chat-input"));
        assert!(!super::is_claude_input("Search", "messageInput_x"));
    }
}
