# Academia

Screenshot answers for **Linux Hyprland**.
Built in Rust. Requests use a local 9Router gateway.

## Start

1. Install `grim` and `notify-send`; run inside a Wayland/Hyprland session.
2. Start 9Router and create a combo named `Academic` containing models that support
   image input. Configure their connections and fallback order in 9Router.
3. Copy `.env.example` to `.env`. Set `ROUTER_API_KEY` to a **9Router gateway key**
   if gateway authentication is enabled; otherwise leave it empty.

```dotenv
ROUTER_BASE_URL=http://127.0.0.1:20128/v1
ROUTER_MODEL=Academic
ROUTER_API_KEY=
```

The app sends `model: "Academic"`; 9Router selects models and handles fallbacks.
Provider keys belong in 9Router, not this app. To change the combo, use
`academic --set-model COMBO`. Upstream provider quotas and costs still apply.

```sh
academic                    # Screenshot answer as a notification
academic --clear            # Clear dunst notifications (-c)
academic --simulate         # Preview sample notifications (-s)
academic --notify "hello"   # Send a custom notification
academic --set-model COMBO  # Update ROUTER_MODEL in .env
```

Keep `.env` beside the executable, or set `ACADEMIC_ENV_FILE` to its path.

## Develop

```sh
cargo test                  # Run unit and integration tests
cargo fmt --check           # Check code formatting
cargo clippy -- -D warnings # Lint with Clippy
cargo build --release       # Build optimized binary in target/release/academic
```
