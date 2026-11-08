# CutePlants feed

Static data for the CutePlants device OS in VRChat worlds: the public YouTube, BOOTH and weather feeds, rebuilt every
30 minutes by GitHub Actions and served from GitHub Pages (`https://<owner>.github.io/cuteplants-feed/`). No server.
`*.github.io` is on VRChat's string and image allowlist, so players load it with no setting turned on.

The world bakes every file URL (VRChat can't build URLs at runtime), so the file layout is a contract with
`CPFeedBank` / `CPFeedBankEditor` in the Unity project. It's described at the top of `src/main.rs`.

The X tab isn't here: X shows nothing to logged-out readers, so X stays a desktop-app feed share.

## Run locally

Needs Rust, `curl`, `ffmpeg` and `yt-dlp` on PATH, and the Shared repo checked out next to this one (`../Shared`).

```bash
cargo test
cargo run --release -- public
```

Set `YT_API_KEY` (YouTube Data API v3, free 10k units/day; a run uses 33) for region charts instead of yt-dlp searches.

## Publish

1. Create the public repo `cuteplants-feed` and push this repo to `main`; push Shared as `cuteplants-shared`.
2. Settings → Pages → Source: GitHub Actions.
3. Optional secrets: `YT_API_KEY`, and `SHARED_TOKEN` if Shared is private.
4. In the world: `logiccuteguy.github.io` in the Video Player Allowed Domains (the `v/` pages hand the player on to YouTube).

Weather: MET Norway Locationforecast (CC BY 4.0).
