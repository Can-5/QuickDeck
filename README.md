# QuickDeck

**Universal app, file and shortcut launcher for your desktop.** Press `Alt+Space` anywhere, type, hit `Enter`. No accounts, no cloud, no telemetry.

![Platform](https://img.shields.io/badge/platform-Linux%20%C2%B7%20Windows%20%C2%B7%20macOS-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Version](https://img.shields.io/badge/version-0.1.0-orange)

## What it is

QuickDeck is a fast, keyboard-first spotlight-style launcher built with Tauri 2 (Rust shell, Svelte + TypeScript UI). It indexes installed applications, searches your files, opens websites and runs your own custom commands — all from one frameless window.

## Features

- **Global hotkey** `Alt+Space` toggles the window (registration failure is logged, not fatal)
- **App search** — Linux: XDG `.desktop` scan (`NoDisplay`/`Hidden` filtered, launched via `gtk-launch` with a no-shell `Exec` fallback); Windows: Start Menu/Desktop shortcuts; macOS: `.app` bundles with `Info.plist` display-name parsing
- **File search** — walks `$HOME` with hidden dirs and `node_modules`/`target`/`.git` etc. skipped, depth ≤ 6, 900 ms / 250k-node caps, ranked (exact > prefix > contains)
- **Website shortcuts** — add/remove from the UI (`http(s)` + dot-host or localhost validation; `javascript:` rejected)
- **Command shortcuts** — your own aliases (`> git pull`), run via `sh -c` / `cmd /c` (they are *your* commands, entered by you)
- **Favorites** (sorted first, boost 1000) and **recency/frequency-based ranking** — frecency bonus capped so you need recent, repeated use to top the list
- **Usage history** — records launches (id/name/kind/ts/count); `Recent` section when the query is empty
- **Full keyboard control** — `↑↓` navigate, `Enter` launch, `Ctrl+Enter` favorite, `Esc` close
- **Light / dark / system theme**, persisted; frameless always-on-top window, hidden on blur

## Install

Prerequisites: [Node.js](https://nodejs.org) ≥ 20, [pnpm](https://pnpm.io) ≥ 9, [Rust](https://rustup.rs) (stable), and the [Tauri 2 system dependencies](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
git clone <this-repo> quickdeck && cd quickdeck
pnpm install
pnpm tauri dev      # run in development mode
pnpm tauri build    # produce an installer/bundle for your OS
```

Build artifacts land in `src-tauri/target/release/bundle/`.

## Development

```bash
pnpm install        # install frontend deps
pnpm test           # unit tests (vitest) — ranking, frecency, grouping
pnpm build          # production frontend build
cd src-tauri && cargo test && cargo build
```

CI (GitHub Actions, `.github/workflows/ci.yml`) runs frontend tests + build on every push/PR, plus a `tauri build` bundle job on Ubuntu, Windows and macOS.

### Project layout

```
├── src/
│   ├── App.svelte      # launcher UI: search, results, add-website/command modals
│   └── lib/rank.ts     # pure ranking: match quality × frecency × favorite boost
├── src-tauri/src/
│   ├── apps.rs         # app discovery (Linux/Windows/macOS) + launch helpers
│   ├── search.rs       # bounded $HOME file walk + path safety checks
│   └── store.rs        # single JSON config store (favorites, websites, commands, history)
```

## Privacy & security

- No network access for core functionality — the CSP allows only `'self'` + Tauri IPC. Websites you add are opened in your browser, never proxied.
- Launching uses `gtk-launch` or direct `Command::spawn` with a tokenized, field-code-stripped `Exec` — **no shell** is involved for apps or files.
- Website/command entries are validated (scheme allow-list, name/length caps) and stored in your OS config dir (`<app_config>/quickdeck.json`), atomically.
- File opening requires an absolute path; command shortcuts run exactly what you typed, nothing else.

## Platform notes & limitations

- **Linux (KDE Plasma, Wayland tested)**: `gtk4`/`gtk-launch` come with the desktop; if `gtk-launch` is missing the app falls back to a direct spawn (some desktop actions like `%U` handlers may behave differently).
- **Windows/macOS**: app lists show shortcut names / bundle names; `.desktop`-style keywords are unavailable.
- The launcher window has no taskbar entry by design (spotlight behavior) and hides on blur.

## License

MIT — see [LICENSE](./LICENSE).
