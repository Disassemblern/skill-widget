# skill-widget

A tiny always-on-top window that ranks your installed Claude Code skills by how relevant they are to what you are typing, so you can see the right skill before you send a message.

- **Live while you type** in Claude Code's VS Code chat box (Windows), and again when you send.
- **Every skill, always listed.** Only the order changes; scroll to see the rest.
- **Short description under each skill**; hover for the full text and what its plugin does.
- **Click a skill** to copy `/skill-name`, then paste it into Claude.
- **Local and private.** Embeddings run on your machine with a small ONNX model. Nothing is sent anywhere unless you point it at an HTTP embedding server yourself.

## What it lists

Every skill Claude or you can call:

- personal skills and commands in `~/.claude/skills` and `~/.claude/commands`
- skills and commands of **enabled** plugins
- claude.ai-synced skills
- project skills and commands in `<project>/.claude/` (the folder of the session you are typing in, and its parents)

Skills that a plugin injects into every session through its `SessionStart` hook (for example `ponytail:ponytail` or `superpowers:using-superpowers`) are hidden automatically: they are already active, so suggesting them is noise. You can hide more in the config.

## Install

Requires Windows 10/11, the [Rust toolchain](https://rustup.rs) and the Visual Studio C++ build tools. Other platforms build but are untested, and live typing is Windows-only there.

```sh
cargo install --git https://github.com/Disassemblern/skill-widget
skill-widget install     # adds one UserPromptSubmit hook to ~/.claude/settings.json (backup kept)
skill-widget             # opens the widget
```

Restart open Claude Code sessions after `skill-widget install`. The first launch downloads the default model (about 130 MB) into `~/.claude/skill-widget/models`.

## Use

- **Type in Claude's chat box.** The list re-sorts about twice a second.
- **Send a message.** The list ranks against what you sent, and project skills for that folder are picked up.
- **Type in the widget's own box** to try a query without Claude.
- **Click** a skill to copy it. Tags: `(manual)` means only you can call it; `(project)` means it comes from the current project.

## Switching the embedding model

Edit `~/.claude/skill-widget/config.toml` and save. The running widget reloads it.

```toml
backend = "fastembed"     # local model; `skill-widget models` lists the built-in names
model = "BGESmallENV15"
query_prefix = ""         # some models want prefixes, e.g. e5: "query: " / "passage: "
doc_prefix = ""

# Any OpenAI-compatible /v1/embeddings server (Ollama, LM Studio, llama.cpp, OpenAI...):
# backend = "http"
# url = "http://localhost:11434/v1/embeddings"
# model = "nomic-embed-text"
# api_key_env = "OPENAI_API_KEY"   # the NAME of an env var; never put the key itself here

hide = []                 # skills never to show, e.g. ["ponytail:ponytail-help"]
```

Skill vectors are cached per model, so switching back to a model you tried is instant.

### Compare models with numbers

`eval.toml` maps prompts to the skill that should rank first. Replace the examples with your own real prompts, then:

```sh
skill-widget eval                       # the configured model
skill-widget eval --model AllMiniLML6V2 # any other built-in model
```

It prints top-1 / top-3 hit rates and MRR. Cases for skills you do not have are skipped.

## How it works

| Piece | What it does |
|---|---|
| `skill-widget hook` | Run by Claude Code on every sent message. Saves the message and the session folder to `~/.claude/skill-widget/prompt.json`. Prints nothing, so it adds nothing to Claude's context. |
| Live draft (`src/draft.rs`) | Reads the text of Claude's chat box through Windows UI Automation, the interface screen readers use. Only the focused element is checked, and only when it is Claude's "Message input" box. Nothing else is read and nothing is stored. |
| Discovery (`src/discover.rs`) | Finds skills from the sources above and re-scans on every sent message, so new installs show up without a restart. |
| Ranking (`src/engine.rs`) | Embeds `name: description` for each skill once (cached), embeds the query, and sorts by cosine similarity. About 5-10 ms per query. |

## Uninstall

```sh
skill-widget uninstall   # removes the hook from ~/.claude/settings.json
cargo uninstall skill-widget
```

Then delete `~/.claude/skill-widget` (config, cache, models).

## Limits

- Live typing depends on how the Claude Code VS Code extension labels its chat box. An extension update could rename it; ranking on send keeps working either way.
- With several Claude sessions open, the widget follows the most recent message.
- Built-in commands such as `/code-review` have no skill file and are not listed.
