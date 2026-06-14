# BelloBox — Features

BelloBox is a macOS menu-bar **toolbox for the text you have selected**. Select
text in any app and a small floating toolbar appears next to it; each button is a
tool. Results can be copied or pasted straight back in place.

This document lists what BelloBox can do and how you interact with each feature.

---

## The core interaction

1. **Select text** in any app — a note, an email, a code comment, a web form.
2. A small **floating toolbar** appears just above your selection with one button
   per tool: **AI ✨**, **QR ▦**, and **Text Tools 🔧**.
3. **Click a tool** (or dismiss by clicking elsewhere). A popup opens next to the
   selection.
4. Act on the result — most tools offer **Copy** and **Replace** (Replace pastes
   the result back over your original selection in the source app).

You can also summon the AI tool on the current selection from anywhere with the
global hotkey **⌃⌥⌘B**, or run any tool from the menu-bar menu.

> The toolbar and popups never steal focus from the app you're working in, so
> your selection stays put.

---

## Tool 1 — AI Assistant ✨

Ask a configurable AI to act on the selected text. The answer **streams in live**.

**One-click actions:**
- Fix Spelling & Grammar
- Improve Writing
- Make Shorter
- Professional Tone
- Friendly Tone
- Summarize
- Explain
- Translate to English

**Custom instruction:** type anything into the "Ask BelloBox to…" field
(e.g. "make this a bullet list", "rephrase as a friendly reply") and run it.

**After a result:**
- **Copy** the result to the clipboard, or
- **Replace** your original selection with it (⌘↩).

**Bring your own AI** (set up once in Settings or onboarding):
- **API format:** OpenAI-compatible, Anthropic-compatible, or the local **Codex CLI**.
- **Endpoint:** any base URL — works with OpenAI, Anthropic, OpenRouter, Groq,
  together.ai, and local servers like Ollama or LM Studio.
- **Codex CLI:** runs your local `codex` via your login shell (so it matches your
  terminal — same codex, same config) with your existing Codex login. No API key
  and no path to set; an optional command field is there if you need a specific
  binary.
- **Load models:** fetch the available models from the endpoint and pick one
  (Codex offers preset models).
- **Model** and **API key** (the key is stored in your macOS Keychain).
- **System prompt:** editable text that shapes every transformation.
- **Test connection:** runs a quick "say hi" to confirm it works.

---

## Tool 2 — QR Code ▦

Turn the selection into a scannable QR code.

- A **live QR code** renders from the selected text.
- An **editable text field** lets you tweak the content; the QR updates as you
  type.
- **Copy** the QR image to the clipboard, or **Save…** it as a PNG.

Great for sharing a link, address, Wi-Fi string, or any text to a phone.

---

## Tool 3 — Text Tools 🔧

Offline utilities — no AI key, no network, instant. Pick a category at the top of
the popup; the input is pre-filled with your selection and is editable. Most
categories offer **Copy** and **Replace**.

- **Case** — UPPERCASE, lowercase, Title Case, Sentence case, camelCase,
  PascalCase, snake_case, kebab-case, CONSTANT_CASE.
- **Encode** — Base64, URL, HTML entities, Hex.
- **Decode** — **auto-detects** Base64 / URL / HTML / Hex (with a manual override
  if it guesses wrong).
- **Pretty** — **auto-detects** the language and reindents: JSON, XML/HTML, and
  brace-style code (CSS / JS).
- **Hash** — MD5, SHA-1, SHA-256, SHA-512, shown together; copy any.
- **Lines** — sort A→Z / Z→A, reverse, remove duplicates, remove empty lines,
  trim each line.
- **Count** — characters, characters without spaces, words, lines, and a
  **model-aware token estimate**. The token model can be changed **right in the
  popup** (provider toggle + model field + preset menu); the estimate updates
  live and the choice is remembered.

> The token count is a fast, word-aware estimate (not an exact tokenizer), and it
> shows which model and tokenizer family it is based on.

---

## Home window

Opening BelloBox (from Finder or Launchpad, or **Open BelloBox** in the menu)
shows a home window with:

- The app version and a one-line description.
- Live **status**: whether an AI provider is configured and whether Accessibility
  is granted, each with a quick action to fix it.
- A short how-to.
- Buttons for **Settings**, the **Setup Guide**, and **Check for Updates**.

It opens centered, and re-opening the app brings it back (handy, since a menu-bar
app has nothing in the Dock).

## Appearance / Themes

- Choose **System**, **Light**, or **Dark** in **Settings → Appearance**.
- "System" follows your macOS setting; Light/Dark force BelloBox either way.

## Onboarding

- On first launch a short guide walks you through what BelloBox does, granting
  Accessibility, and connecting your AI provider (load the model list and run a
  "say hi" test to confirm it works).
- You can **Skip** the setup and configure it later.
- Reopen it anytime from the menu bar → **Set Up BelloBox…**.

## Menu bar

The ✨ menu-bar icon gives you:
- **Open BelloBox** (the home window)
- **Ask BelloBox About Selection**, **Generate QR Code from Selection**,
  **Text Tools on Selection**
- **Set Up BelloBox…**, **Settings…**
- **Check for Updates…**
- **Quit**

## Updates

BelloBox updates itself via [Sparkle](https://sparkle-project.org). Use
**Check for Updates…**, or let automatic checks handle it.

## Permissions & privacy

- BelloBox needs **Accessibility** access to read the selected text and to paste
  replacements. It only reads the selection when you ask it to.
- Text is sent only to the **AI endpoint you configure** — and the AI tool is the
  only feature that uses the network. QR and Text Tools run entirely offline.
- Your API key is stored in the macOS **Keychain**.

## Requirements

- macOS 13 (Ventura) or later.
- Your own OpenAI-compatible or Anthropic-compatible endpoint + key for the AI
  tool (the other tools work without one).
