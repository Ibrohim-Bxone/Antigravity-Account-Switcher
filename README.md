# Antigravity Account Switcher Pro ⚡

> **A secure, high-performance Windows desktop application built with Tauri 2.x and Rust to seamlessly manage, auto-switch, and monitor quotas across multiple Google Antigravity 2.0, Claude Pro, and ChatGPT Plus accounts.**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](#development)
[![Tauri 2.x](https://img.shields.io/badge/Tauri-2.x-blue.svg)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-MSVC-orange.svg)](https://www.rust-lang.org/)
[![Safety](https://img.shields.io/badge/Process%20Guardian-Active-success.svg)](#process-guardian)

---

## 🇺🇿 O'zbekcha Tavsif (Summary in Uzbek)

**Antigravity Account Switcher** — Google Antigravity 2.0 (Gemini Pro), Claude Pro (Cursor / Opus 5.5) va ChatGPT Plus (Codex / GPT-4o) hisoblaridagi limitlar, 5-soatlik va haftalik kvotalarni avtomatik nazorat qiluvchi va uzluksiz almashtirib beruvchi professional Windows desktop ilovasi.

### 🔥 Asosiy Imkoniyatlar:
1. **Process Guardian Shield (Jarayonlar Himoyachisi):**
   - Hech qachon orqa fonda ishlab turgan vazifalar, subagentlar, kompilyatsiya (`cargo`, `node`, `powershell`) yoki faol suhbat paytida hisobni majburiy o'chirib yubormaydi!
   - Ishlar to'liq yakunlanib, tizim tinch (idle) holatga kelgandagina xavfsiz o'tadi (Deferred Safe Switch).
2. **Pre-flight Live Token Health Check:**
   - Yaroqsiz yoki Google tomonida "Verification" (qayta kirish) talab qiladigan hisoblarni avtomatik aniqlaydi va ularga o'tishni bloklaydi. Faqatgina 100% sog'lom va tasdiqlangan hisoblargagina o'tadi.
3. **Sessiya va Chat Tarixi Uzluksizligi:**
   - Hisob almashtirilganda ham ochiq turgan chat oynasi, loyiha va `brain/` xotirasi to'liq saqlanib qoladi. Yangi hisob avvalgi suhbat kontekstini to'liq davom ettiradi.
4. **Dinamik 5-Soatlik va Haftalik Rolling Limitlar:**
   - Claude va ChatGPT hisoblarining 5-soatlik oynasi va aniq tiklanish vaqti (`Resets at 7:11 PM` yoki `Resets in 4 hr 17 min`) real vaqtda orqaga hisoblab boriladi va vaqt yetganda avtomatik 100% ga tiklanadi.
5. **In-Window Seamless Mini Mode:**
   - Alohida oq ramkasiz, to'g'ridan-to'g'ri asosiy oynani ixcham (320x160) mini-vidjetga aylantirish.
6. **100% Maxfiylik va Xavfsizlik:**
   - Hech qanday shaxsiy email, parol yoki API kalitlar repoda saqlanmaydi. Windows DPAPI (`CryptProtectData`) va Credential Manager orqali lokal shifrlanadi.

---

## 🇬🇧 English Documentation

### Key Features

*   **Process Guardian**: Monitors Antigravity process trees, subagents, and background terminal workers (`powershell`, `cargo`, `git`, `python`, `node`). Prevents abrupt shutdowns or switches while computational jobs are in progress.
*   **Token Health Pre-Flight**: Live-validates OAuth tokens prior to activating an account. Skips accounts flagged for security challenges or requiring browser re-verification.
*   **Session & Context Continuity**: Preserves workspace databases and `.gemini/antigravity/brain` transcripts so ongoing AI reasoning sessions seamlessly resume without context loss.
*   **Dual-Quota & External AI Monitoring**: Real-time 5-hour rolling recovery windows, weekly allocations, and cloud session credits for Claude Pro (Opus 5.5) and ChatGPT Plus (Codex / GPT-4o).
*   **Seamless In-Window Mini Mode**: Instant transformation into an ultra-compact widget with always-on-top pinning and fast single-click account swaps.
*   **DPAPI Enterprise Security**: Inactive profiles are encrypted on disk using Windows Data Protection API (DPAPI) tied to the active Windows user context.

---

## Switching Levels (Restart Modes)

| Level | Name | Est. Time | Speed | Description |
|---|---|---|---|---|
| **Level 1** | Full Restart | ~17s | Baseline | Gracefully closes and restarts the entire application. |
| **Level 1+** | Optimized Restart | ~8s | **3x Faster** | Closes GUI gracefully while instantly stopping idle workers. |
| **Level 2** | Reload | ~5s | **4x Faster** | Restarts only the internal `language_server.exe` process. |
| **Level 2+** | Fast Reload | ~3s | **6x Faster** | Patches `app.asar` to reduce language server cooldown timers. |

---

## 🛠️ Building & Development

### Prerequisites
*   Windows 10 / 11 (64-bit)
*   [Node.js](https://nodejs.org/) (v18+) & `npm`
*   [Rust](https://rustup.rs/) (stable-x86_64-pc-windows-msvc)
*   Microsoft Visual Studio C++ Build Tools

### Installation & Run

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/Ibrohim-Bxone/Antigravity-Account-Switcher.git
    cd Antigravity-Account-Switcher
    ```

2.  **Install dependencies:**
    ```powershell
    npm install
    ```

3.  **Start development server:**
    ```powershell
    npm run tauri dev
    ```

4.  **Build production release binary:**
    ```powershell
    npx tauri build --no-bundle
    ```
    Use the Tauri CLI, not plain `cargo build --release`: only the CLI enables the
    `custom-protocol` feature that embeds `dist/` into the binary for production.

The compiled binary will be located at `target/release/app.exe`.

---

## ⚖️ Legal Disclaimer

This application is provided strictly for personal workflow optimization and research purposes. Users are responsible for adhering to all relevant third-party Terms of Service, including the [Google Terms of Service](https://policies.google.com/terms) and the [Gemini API Terms of Service](https://ai.google.dev/gemini-api/terms).

> [!CAUTION]
> Programmatically switching accounts to get around usage limits or quotas may violate Google's policies and can lead to account suspension. The developers and contributors are not liable for any loss of data, loss of access, account bans or service disruption arising from use of this software. You run it at your own risk.

## 🙏 Credits

This project is a fork of [Ximeeek/Antigravity-Account-Switcher](https://github.com/Ximeeek/Antigravity-Account-Switcher), released under the MIT License. The quota monitoring, Process Guardian and external AI account features were added in this fork.

---

## 📄 License

Distributed under the [MIT License](LICENSE).
