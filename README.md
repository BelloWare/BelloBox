# BelloBox

A tiny macOS menu-bar assistant for the text you already have in front of you.

Select text in **any** app — a note, an email, a code comment, a web form — and a
small BelloBox button appears next to your selection. Click it (or press
**⌃⌥⌘B**) and a popup opens where you can:

- **Fix Spelling & Grammar**, **Improve Writing**, **Make Shorter**, switch to a
  **Professional** or **Friendly** tone, **Summarize**, **Explain**, or
  **Translate to English** — one click each.
- Type your own instruction ("make this a bullet list", "rephrase as a tweet", …).
- Watch the answer stream in, then **Copy** it or **Replace** the original
  selection in place.

## Bring your own AI

BelloBox does not ship a model or an API key. In **Settings** you choose:

- **API format** — OpenAI-compatible or Anthropic-compatible.
- **Endpoint** — any base URL. Works with the OpenAI and Anthropic APIs as well
  as OpenRouter, Groq, together.ai, and local servers like Ollama or LM Studio.
- **Model** and **API key** (the key is stored in your macOS Keychain).
- The **system prompt** that shapes every transformation.

A **Test connection** button verifies your configuration.

## Permissions

BelloBox needs **Accessibility** access to read the selected text and paste
replacements back. macOS will prompt on first launch, or grant it under
*System Settings → Privacy & Security → Accessibility*.

## Requirements

- macOS 13 (Ventura) or later.

## Updates

BelloBox updates itself via [Sparkle](https://sparkle-project.org). Use
*Check for Updates…* in the menu-bar menu.

---

© 2026 Belloware.
