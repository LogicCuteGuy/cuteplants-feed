# cuteplants-feed

Static data for the CutePlants device OS in VRChat worlds: the public YouTube, BOOTH, X and weather feeds, rebuilt every
30 minutes by GitHub Actions and served from GitHub Pages (`https://<owner>.github.io/cuteplants-feed/`). No server.
`*.github.io` is on VRChat's string and image allowlist, so players load it with no setting turned on.

The world bakes every file URL (VRChat can't build URLs at runtime), so the file layout is a contract with
`CPFeedBank` / `CPFeedBankEditor` in the Unity project. It's described at the top of `src/main.rs`.

**X:** the timeline of `@LogicCuteGuy` (posts and reposts, no replies, nothing marked sensitive) through FxTwitter's
public API, since X itself shows nothing to logged-out readers. Another account: set the `X_USER` env var (or repo variable).

There is no search: static hosting can't answer arbitrary queries, so search is a desktop-app feature.

## Run locally

Needs Rust, `curl`, `ffmpeg` and `yt-dlp` on PATH. Self-contained: `src/ui.rs` is this repo's copy of the CutePlants
widget builder (the command format the world's `CPDevice.DrawUi` reads).

```bash
cargo test
cargo run --release -- public
```

Set `YT_API_KEY` (YouTube Data API v3, free 10k units/day; a run uses 33) for region charts instead of yt-dlp searches.

## Publish

1. Push this repo to the public GitHub repo `cuteplants-feed` (`main`).
2. Settings → Pages → Source: GitHub Actions.
3. Optional secret: `YT_API_KEY`.
4. In the world: `logiccuteguy.github.io` in the Video Player Allowed Domains (the `v/` pages hand the player on to YouTube).

Weather: MET Norway Locationforecast (CC BY 4.0).
