# Stealth

Screenshot answers for **Linux Hyprland**.
Built in Rust. Powered by Antigravity CLI headless (`agy`) as the primary engine with Google Gemini API key fallback.

## Start

1. Install `grim` and `notify-send`; run inside a Wayland/Hyprland session.
2. Ensure `agy` (Antigravity CLI) is installed and authenticated.
3. (Optional) Provide a `GEMINI_API_KEY` in `.env` to serve as a high-speed fallback if `agy` fails or is unreachable.
4. Copy `.env.example` to `.env` and adjust the model and reasoning effort:

```dotenv
# Antigravity CLI Headless (Main Engine)
AGY_MODEL=gemini-3.8-flash-low
AGY_EFFORT=low

# Google Gemini API Key (Fallback Engine)
GEMINI_API_KEY=
GEMINI_MODEL=gemini-2.5-flash
# GEMINI_BASE_URL=https://generativelanguage.googleapis.com
```

### CLI Usage

```sh
stealth                    # Capture screen, solve via agy (or Gemini fallback), and notify
stealth --clear            # Clear dunst notifications (-c)
stealth --simulate         # Preview sample notifications (-s)
stealth --notify "hello"   # Send a custom notification
stealth --set-model MODEL  # Update AGY_MODEL in .env
stealth --set-effort EFFORT# Update AGY_EFFORT (low|medium|high|xhigh|max) in .env
```

Keep `.env` beside the executable, or set `STEALTH_ENV_FILE` to its path.

## Develop

```sh
cargo test                  # Run unit and integration tests
cargo fmt --check           # Check code formatting
cargo clippy -- -D warnings # Lint with Clippy
cargo build --release       # Build optimized binary in target/release/stealth
```
