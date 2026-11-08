//! cp-feed: builds the CutePlants public feed as static files for GitHub Pages (no server of our own).
//! Run by GitHub Actions every 30 min; the world bakes the file URLs (CPFeedBank) because VRChat can't build URLs at
//! runtime, and *.github.io is on VRChat's string + image allowlist, so every player can load it with no setting.
//!
//!   yt/{CC}/{tab}.json   public YouTube tab pages per region (home, music, gaming)   booth/{tab}.json (home, new)
//!   p/{P}.json           every page (P fixed, see `plan`): search results open their page from here
//!   a/{P}.jpg            4×4 thumbnail atlas of page P (row 0 at the top)
//!   v/{nnnn}.html        meta-refresh to the video of item n = P*16 + i (yt-dlp, which VRChat's player uses, follows it)
//!   i/{nnnn}.jpg         large BOOTH picture of item n (the image loader refuses redirects, so the bytes live here)
//!   s/yt/{CC}.json+.jpg  search catalog of a region: every item of its pages + one atlas of their thumbnails
//!   s/booth.json+.jpg    the same for BOOTH
//!   wx/{CC}.txt          "☀ 31°C\tBangkok": weather for the region's city (MET Norway)
//!
//! YouTube: the YouTube Data API when YT_API_KEY is set (most popular per region, music / gaming categories),
//! else yt-dlp searches in the region's language sorted by this week's views. BOOTH: listing pages + item JSON.
//! Tools on PATH: curl, ffmpeg, yt-dlp (without a key). Usage: cp-feed [out dir, default "public"]

mod feed_ui;

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::{json, Value};

pub const PER_PAGE: usize = 16;
/// Pages the world bakes URLs for (CPFeedBank.Pages must match).
pub const PAGES: usize = 80;

/// One country per YouTube language group (CPFeedBake.Regions in the world must list the same codes, same order).
pub const REGIONS: [&str; 11] = ["US", "JP", "KR", "TW", "SG", "TH", "RU", "ES", "BR", "FR", "DE"];
pub const YT_TABS: [&str; 3] = ["home", "music", "gaming"];
pub const BOOTH_TABS: [&str; 2] = ["home", "new"];
/// Pages per YouTube region: one per tab, then one more per tab (items 17–32) for the search catalog.
const YT_PAGES: usize = 6;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub title: String,
    pub by: String,
    pub views: i64,
    pub dur: i64,      // seconds, 0 = unknown
    pub live: bool,
    pub price: String, // BOOTH
    pub thumb: String, // small image for the atlas
    pub video: String, // what v/{n}.html points at
    pub image: String, // what i/{n}.jpg holds
}

/// First page number of a source block: YouTube region r, or BOOTH (after every region).
pub fn first_page(src: &str, region: usize) -> usize { if src == "yt" { region * YT_PAGES } else { REGIONS.len() * YT_PAGES } }

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "public".into()));
    let _ = fs::remove_dir_all(&out);
    for d in ["p", "a", "v", "i", "s/yt", "wx", "booth"] { fs::create_dir_all(out.join(d)).expect("out dir"); }
    for cc in REGIONS { fs::create_dir_all(out.join("yt").join(cc)).expect("out dir"); }
    let key = std::env::var("YT_API_KEY").ok().filter(|k| !k.is_empty());

    std::thread::scope(|s| {
        for (r, cc) in REGIONS.iter().enumerate() {
            let (out, key) = (&out, key.as_deref());
            s.spawn(move || {
                let lists: Vec<Vec<Item>> = (0..YT_TABS.len()).map(|k| youtube_list(cc, k, key).unwrap_or_else(|e| { eprintln!("yt {cc} {}: {e}", YT_TABS[k]); vec![] })).collect();
                block(out, "yt", first_page("yt", r), &lists, &YT_TABS, &format!("yt/{cc}"), &format!("s/yt/{cc}"));
            });
        }
        let out = &out;
        s.spawn(move || {
            let lists: Vec<Vec<Item>> = BOOTH_TABS.iter().map(|t| booth_list(t).unwrap_or_else(|e| { eprintln!("booth {t}: {e}"); vec![] })).collect();
            block(out, "booth", first_page("booth", 0), &lists, &BOOTH_TABS, "booth", "s/booth");
        });
        s.spawn(move || weather_all(out));
    });
    fs::write(out.join("index.html"), "<!doctype html><meta charset=utf-8><title>CutePlants feed</title><p>Data for the CutePlants device OS in VRChat worlds. Weather: MET Norway (CC BY 4.0).\n").ok();
    fs::write(out.join(".nojekyll"), "").ok();
}

/// One source block: tab pages (first 16 of each list), catalog pages (items 17–32), the tab files, the catalog.
fn block(out: &Path, src: &str, p0: usize, lists: &[Vec<Item>], tabs: &[&str], tab_dir: &str, catalog: &str) {
    let mut pages: Vec<(usize, Vec<Item>)> = Vec::new();
    for (k, list) in lists.iter().enumerate() {
        pages.push((p0 + k, list.iter().take(PER_PAGE).cloned().collect()));
    }
    for (k, list) in lists.iter().enumerate() {
        pages.push((p0 + tabs.len() + k, list.iter().skip(PER_PAGE).take(PER_PAGE).cloned().collect()));
    }
    for (p, items) in &pages {
        assert!(*p < PAGES, "page {p} past CPFeedBank.Pages");
        page_files(out, src, *p, items);
    }
    for (k, t) in tabs.iter().enumerate() {
        fs::write(out.join(format!("{tab_dir}/{t}.json")), page_json(src, pages[k].0, &pages[k].1)).ok();
    }
    catalog_files(out, src, &pages, catalog);
}

/// Page JSON the world parses with VRCJson: items + the page's screens built with the app SDK (`ui`).
pub fn page_json(src: &str, page: usize, items: &[Item]) -> String {
    let items_json: Vec<Value> = items
        .iter()
        .map(|i| json!({ "t": i.title, "by": i.by, "views": i.views, "dur": i.dur, "live": i.live, "price": i.price, "k": kind(i) }))
        .collect();
    json!({ "page": page, "items": items_json, "ui": feed_ui::screens(src, items) }).to_string()
}

fn kind(i: &Item) -> &'static str { if !i.video.is_empty() { "v" } else if !i.image.is_empty() { "i" } else { "" } }

fn page_files(out: &Path, src: &str, p: usize, items: &[Item]) {
    fs::write(out.join(format!("p/{p}.json")), page_json(src, p, items)).ok();
    let cell = if src == "yt" { (320, 180) } else { (256, 256) };
    let thumbs: Vec<String> = items.iter().map(|i| i.thumb.clone()).collect();
    if let Err(e) = atlas(&thumbs, cell, &out.join(format!("a/{p}.jpg"))) { eprintln!("atlas {p}: {e}"); }
    for (i, it) in items.iter().enumerate() {
        let n = p * PER_PAGE + i;
        if !it.video.is_empty() { fs::write(out.join(format!("v/{n:04}.html")), redirect_html(&it.video)).ok(); }
        else if !it.image.is_empty() {
            if let Ok(b) = curl(&it.image) { fs::write(out.join(format!("i/{n:04}.jpg")), b).ok(); }
        }
    }
}

/// A page that sends yt-dlp (VRChat's video resolver) on to the video. Links are built here from checked ids.
pub fn redirect_html(url: &str) -> String {
    let u = url.replace('&', "&amp;").replace('"', "&quot;").replace('<', "");
    format!("<!doctype html><meta charset=utf-8><meta http-equiv=\"refresh\" content=\"0; url={u}\"><link rel=canonical href=\"{u}\">\n")
}

/// Search catalog: one entry per item (title, channel / shop, page, index, cell in the catalog atlas) and an atlas
/// that tiles the pages' atlases at half size (pages in a row of `tiles` across). The world filters it locally.
fn catalog_files(out: &Path, src: &str, pages: &[(usize, Vec<Item>)], name: &str) {
    let tiles = if src == "yt" { 3 } else { 2 };
    let cols = tiles * 4;
    let rows = pages.len().div_ceil(tiles) * 4;
    let mut items = Vec::new();
    for (k, (p, list)) in pages.iter().enumerate() {
        for (i, it) in list.iter().enumerate() {
            let cell = ((k / tiles) * 4 + i / 4) * cols + (k % tiles) * 4 + i % 4;
            items.push(json!({ "t": it.title, "by": it.by, "m": catalog_meta(src, it), "p": p, "i": i, "c": cell, "b": if it.live { String::new() } else { feed_ui::length(it.dur) } }));
        }
    }
    fs::write(out.join(format!("{name}.json")), json!({ "cols": cols, "rows": rows, "items": items }).to_string()).ok();
    let atlases: Vec<PathBuf> = pages.iter().map(|(p, _)| out.join(format!("a/{p}.jpg"))).collect();
    let (w, h) = if src == "yt" { (640, 360) } else { (512, 512) };
    if let Err(e) = tile(&atlases, tiles, (w, h), &out.join(format!("{name}.jpg"))) { eprintln!("catalog atlas {name}: {e}"); }
}

/// The result row's second line, already in the world's label format (untrusted text made inert).
fn catalog_meta(src: &str, it: &Item) -> String {
    use cuteplants_sdk::ui::literal;
    if src == "booth" {
        let wish = if it.views >= 0 { format!("  ♡ {}", feed_ui::compact_num(it.views)) } else { String::new() };
        return format!("<b>{}</b>{wish}", literal(&it.price));
    }
    let by = literal(&it.by);
    let sep = if by.is_empty() { "" } else { " · " };
    if it.live { return format!("{by}{sep}● {{@feed.live}}"); }
    if it.views >= 0 { format!("{by}{sep}{{@feed.views|{}}}", feed_ui::compact_num(it.views)) } else { by }
}

// ---------- sources ----------

fn run(cmd: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new(cmd).args(args).output().map_err(|e| format!("{cmd}: {e}"))?;
    if !out.status.success() { return Err(format!("{cmd} failed: {}", String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or(""))); }
    Ok(out.stdout)
}

fn curl(url: &str) -> Result<Vec<u8>, String> {
    run("curl", &["-sSfL", "--max-time", "20", "--retry", "2", "-A", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) CutePlants", url])
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// YouTube interface language and the words for the Home / Music / Gaming tabs in the region's main language.
pub fn region_words(cc: &str) -> (&'static str, [&'static str; 3]) {
    match cc {
        "JP" => ("ja", ["動画", "ミュージックビデオ", "ゲーム実況"]),
        "KR" => ("ko", ["영상", "뮤직비디오", "게임"]),
        "TW" => ("zh-TW", ["影片", "MV", "遊戲"]),
        "SG" => ("zh-CN", ["视频", "MV", "游戏"]),
        "TH" => ("th", ["คลิป", "เพลง", "เกม"]),
        "RU" => ("ru", ["видео", "клип", "игры"]),
        "ES" => ("es", ["video", "videoclip", "videojuegos"]),
        "BR" => ("pt", ["vídeo", "clipe", "jogos"]),
        "FR" => ("fr", ["vidéo", "clip officiel", "jeux vidéo"]),
        "DE" => ("de", ["Video", "Musikvideo", "Let's Play"]),
        _ => ("en", ["video", "music video", "gameplay"]),
    }
}

/// Up to 32 items for tab k of a region: the Data API with a key, else (or when the API refuses) yt-dlp.
fn youtube_list(cc: &str, k: usize, key: Option<&str>) -> Result<Vec<Item>, String> {
    if let Some(key) = key {
        match youtube_api(cc, k, key) { Ok(v) if !v.is_empty() => return Ok(v), Ok(_) => {}, Err(e) => eprintln!("yt api {cc} {k}: {e}") }
    }
    let (hl, words) = region_words(cc);
    let url = format!("https://www.youtube.com/results?search_query={}&gl={cc}&hl={hl}&sp=CAMSAggD", enc(words[k]));
    let out = run("yt-dlp", &["--flat-playlist", "-J", "--no-warnings", "--playlist-items", "1-44", &url])?;
    Ok(parse_ytdlp(&serde_json::from_slice(&out).map_err(|e| e.to_string())?))
}

fn yt_item(id: &str, title: &str, by: &str, views: i64, dur: i64, live: bool) -> Option<Item> {
    if id.len() != 11 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return None; }
    Some(Item {
        title: title.chars().take(200).collect(),
        by: by.chars().take(80).collect(),
        views, dur, live,
        thumb: format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"),
        video: format!("https://www.youtube.com/watch?v={id}"),
        image: format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
        ..Default::default()
    })
}

pub fn parse_ytdlp(v: &Value) -> Vec<Item> {
    v["entries"].as_array().into_iter().flatten().filter_map(|e| {
        yt_item(
            e["id"].as_str()?, // channels / playlists in results have other id shapes and are dropped
            e["title"].as_str().unwrap_or(""),
            e["channel"].as_str().or(e["uploader"].as_str()).unwrap_or(""),
            e["view_count"].as_i64().or(e["concurrent_view_count"].as_i64()).unwrap_or(-1),
            e["duration"].as_f64().map_or(0, |d| d as i64),
            e["live_status"].as_str() == Some("is_live"),
        )
    }).take(2 * PER_PAGE).collect()
}

/// Most popular in a region: all videos (home), Music (category 10), Gaming (20). 1 quota unit per call.
fn youtube_api(cc: &str, k: usize, key: &str) -> Result<Vec<Item>, String> {
    let cat = ["", "&videoCategoryId=10", "&videoCategoryId=20"][k];
    let hl = region_words(cc).0;
    let url = format!("https://www.googleapis.com/youtube/v3/videos?part=snippet,contentDetails,statistics&chart=mostPopular&regionCode={cc}&hl={hl}&maxResults=32{cat}&key={key}");
    let v: Value = serde_json::from_slice(&curl(&url)?).map_err(|e| e.to_string())?;
    Ok(parse_api(&v))
}

pub fn parse_api(v: &Value) -> Vec<Item> {
    v["items"].as_array().into_iter().flatten().filter_map(|e| {
        let sn = &e["snippet"];
        yt_item(
            e["id"].as_str()?,
            sn["localized"]["title"].as_str().or(sn["title"].as_str()).unwrap_or(""),
            sn["channelTitle"].as_str().unwrap_or(""),
            e["statistics"]["viewCount"].as_str().and_then(|s| s.parse().ok()).unwrap_or(-1),
            iso_secs(e["contentDetails"]["duration"].as_str().unwrap_or("")),
            sn["liveBroadcastContent"].as_str() == Some("live"),
        )
    }).collect()
}

/// "PT1H2M5S" → 3725 (0 when unknown).
pub fn iso_secs(s: &str) -> i64 {
    let (mut total, mut n) = (0i64, 0i64);
    for c in s.chars().skip_while(|c| *c != 'T') {
        match c {
            '0'..='9' => n = n * 10 + c.to_digit(10).unwrap() as i64,
            'H' => { total += n * 3600; n = 0 }
            'M' => { total += n * 60; n = 0 }
            'S' => { total += n; n = 0 }
            _ => n = 0,
        }
    }
    total
}

/// Item ids from a BOOTH listing page, in order, without duplicates.
pub fn booth_ids(html: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for part in html.split("data-product-id=\"").skip(1) {
        let id: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !id.is_empty() && id.len() <= 12 && !ids.contains(&id) { ids.push(id); }
    }
    ids
}

/// One BOOTH item JSON (booth.pm/en/items/{id}.json). Adult items are dropped.
pub fn parse_booth(v: &Value) -> Option<Item> {
    if v["is_adult"].as_bool() != Some(false) { return None; }
    let original = v["images"][0]["original"].as_str()?;
    if !original.starts_with("https://booth.pximg.net/") { return None; }
    Some(Item {
        title: v["name"].as_str()?.chars().take(200).collect(),
        by: v["shop"]["name"].as_str().unwrap_or("").chars().take(80).collect(),
        views: v["wish_lists_count"].as_i64().unwrap_or(-1),
        price: v["price"].as_str().unwrap_or("").to_owned(),
        thumb: original.replacen("booth.pximg.net/", "booth.pximg.net/c/300x300_a2_g5/", 1),
        image: original.replacen("booth.pximg.net/", "booth.pximg.net/c/620x620/", 1),
        ..Default::default()
    })
}

/// Up to 32 VRChat-tagged items: popular (home) or newest (new).
fn booth_list(tab: &str) -> Result<Vec<Item>, String> {
    let sort = if tab == "new" { "sort=new&" } else { "" };
    let mut ids = Vec::new();
    for page in 1..=2 {
        let html = String::from_utf8_lossy(&curl(&format!("https://booth.pm/en/items?{sort}tags%5B%5D=VRChat&page={page}"))?).into_owned();
        for id in booth_ids(&html) { if !ids.contains(&id) { ids.push(id); } }
        if ids.len() >= 3 * PER_PAGE { break; }
    }
    let items: Vec<Item> = std::thread::scope(|s| {
        let jobs: Vec<_> = ids.iter().take(3 * PER_PAGE).map(|id| s.spawn(move || {
            curl(&format!("https://booth.pm/en/items/{id}.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|v| parse_booth(&v))
        })).collect();
        jobs.into_iter().filter_map(|j| j.join().ok().flatten()).take(2 * PER_PAGE).collect()
    });
    if items.is_empty() && !ids.is_empty() { return Err("booth items unreadable".into()); }
    Ok(items)
}

// ---------- images ----------

/// 4×4 atlas of thumbnails (cell w×h, row 0 at the top) as JPEG. Missing thumbnails stay dark.
fn atlas(thumbs: &[String], (w, h): (u32, u32), file: &Path) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("cp-feed-{}", file.file_stem().and_then(|s| s.to_str()).unwrap_or("x")));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let got: Vec<Option<PathBuf>> = std::thread::scope(|s| {
        let jobs: Vec<_> = (0..PER_PAGE).map(|k| {
            let (url, dir) = (thumbs.get(k).cloned().unwrap_or_default(), &dir);
            s.spawn(move || {
                if url.is_empty() { return None; }
                let f = dir.join(format!("{k}.img"));
                curl(&url).ok().and_then(|b| fs::write(&f, b).ok()).map(|_| f)
            })
        }).collect();
        jobs.into_iter().map(|j| j.join().ok().flatten()).collect()
    });
    let mut args: Vec<String> = vec!["-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into()];
    let mut filter = String::new();
    for (k, f) in got.iter().enumerate() {
        match f {
            Some(f) => args.extend(["-i".into(), f.to_string_lossy().into_owned()]),
            None => args.extend(["-f".into(), "lavfi".into(), "-i".into(), format!("color=c=0x22262b:s={w}x{h}:d=1")]),
        }
        filter += &format!("[{k}:v]scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1,format=yuvj420p[c{k}];");
    }
    for k in 0..PER_PAGE { filter += &format!("[c{k}]"); }
    let layout: Vec<String> = (0..PER_PAGE).map(|k| format!("{}_{}", (k % 4) as u32 * w, (k / 4) as u32 * h)).collect();
    filter += &format!("xstack=inputs={PER_PAGE}:layout={}", layout.join("|"));
    args.extend(["-filter_complex".into(), filter, "-frames:v".into(), "1".into(), "-q:v".into(), "4".into(), file.to_string_lossy().into_owned()]);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let r = run("ffmpeg", &refs).map(|_| ());
    let _ = fs::remove_dir_all(&dir);
    r
}

/// Tiles page atlases (`tiles` across, each scaled to w×h) into one catalog atlas. Missing pages stay dark.
fn tile(atlases: &[PathBuf], tiles: usize, (w, h): (u32, u32), file: &Path) -> Result<(), String> {
    let n = atlases.len().div_ceil(tiles) * tiles;
    let mut args: Vec<String> = vec!["-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into()];
    let mut filter = String::new();
    for k in 0..n {
        match atlases.get(k).filter(|f| f.exists()) {
            Some(f) => args.extend(["-i".into(), f.to_string_lossy().into_owned()]),
            None => args.extend(["-f".into(), "lavfi".into(), "-i".into(), format!("color=c=0x22262b:s={w}x{h}:d=1")]),
        }
        filter += &format!("[{k}:v]scale={w}:{h},setsar=1,format=yuvj420p[t{k}];");
    }
    for k in 0..n { filter += &format!("[t{k}]"); }
    let layout: Vec<String> = (0..n).map(|k| format!("{}_{}", (k % tiles) as u32 * w, (k / tiles) as u32 * h)).collect();
    filter += &format!("xstack=inputs={n}:layout={}", layout.join("|"));
    args.extend(["-filter_complex".into(), filter, "-frames:v".into(), "1".into(), "-q:v".into(), "4".into(), file.to_string_lossy().into_owned()]);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run("ffmpeg", &refs).map(|_| ())
}

// ---------- weather ----------

/// The city whose weather a region's players see (lat, lon, name).
pub fn region_city(cc: &str) -> (f64, f64, &'static str) {
    match cc {
        "JP" => (35.68, 139.69, "Tokyo"),
        "KR" => (37.57, 126.98, "Seoul"),
        "TW" => (25.03, 121.57, "Taipei"),
        "SG" => (1.29, 103.85, "Singapore"),
        "TH" => (13.75, 100.50, "Bangkok"),
        "RU" => (55.76, 37.62, "Moscow"),
        "ES" => (40.42, -3.70, "Madrid"),
        "BR" => (-23.55, -46.63, "São Paulo"),
        "FR" => (48.86, 2.35, "Paris"),
        "DE" => (52.52, 13.40, "Berlin"),
        _ => (40.71, -74.01, "New York"),
    }
}

fn weather_all(out: &Path) {
    let ua = format!("cp-feed/0.1 CutePlants ({})", std::env::var("CP_CONTACT").unwrap_or_else(|_| "github.com/logiccuteguy/cuteplants-feed".into()));
    for cc in REGIONS {
        let (lat, lon, city) = region_city(cc);
        let url = format!("https://api.met.no/weatherapi/locationforecast/2.0/compact?lat={lat:.2}&lon={lon:.2}");
        let got = run("curl", &["-sSfL", "--max-time", "15", "-A", &ua, &url]).and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string()));
        match got.ok().and_then(|j| weather_line(&j, cc == "US")) {
            Some(now) => { fs::write(out.join(format!("wx/{cc}.txt")), format!("{now}\t{city}")).ok(); }
            None => eprintln!("weather {cc}: no forecast"),
        }
    }
}

/// "☀ 31°C" from a Locationforecast compact document (the first time step).
pub fn weather_line(json: &Value, fahrenheit: bool) -> Option<String> {
    let d = &json["properties"]["timeseries"][0]["data"];
    let c = d["instant"]["details"]["air_temperature"].as_f64()?;
    let sym = d["next_1_hours"]["summary"]["symbol_code"].as_str().or_else(|| d["next_6_hours"]["summary"]["symbol_code"].as_str()).unwrap_or("");
    let (t, u) = if fahrenheit { (c * 9.0 / 5.0 + 32.0, 'F') } else { (c, 'C') };
    Some(format!("{} {}°{u}", glyph(sym), t.round() as i64))
}

/// MET symbol code → a glyph the world's font chain has (☀ ☁ ☂ ☃ ★).
pub fn glyph(sym: &str) -> char {
    let s = sym.split('_').next().unwrap_or("");
    if s.contains("snow") || s.contains("sleet") { '☃' }
    else if s.contains("rain") || s.contains("thunder") { '☂' }
    else if s == "clearsky" || s == "fair" { if sym.ends_with("_night") { '★' } else { '☀' } }
    else { '☁' }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_plan_fits() {
        assert_eq!(first_page("yt", 0), 0);
        assert_eq!(first_page("yt", 10), 60);
        let booth = first_page("booth", 0);
        assert_eq!(booth, 66);
        assert!(booth + 2 * BOOTH_TABS.len() <= PAGES);
    }

    #[test]
    fn parsers() {
        assert_eq!(iso_secs("PT1H2M5S"), 3725);
        assert_eq!(iso_secs("PT45S"), 45);
        assert_eq!(iso_secs("P0D"), 0);
        let api = json!({"items":[{"id":"abcdefghijk","snippet":{"title":"t","channelTitle":"c","liveBroadcastContent":"none"},
            "statistics":{"viewCount":"1234"},"contentDetails":{"duration":"PT3M12S"}}, {"id":"bad"}]});
        let v = parse_api(&api);
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].views, v[0].dur, v[0].video.as_str()), (1234, 192, "https://www.youtube.com/watch?v=abcdefghijk"));
        assert_eq!(booth_ids("data-product-id=\"12\" x data-product-id=\"12\" data-product-id=\"7\""), vec!["12", "7"]);
        let b = json!({"is_adult":false,"name":"n","shop":{"name":"s"},"price":"¥500","wish_lists_count":3,"images":[{"original":"https://booth.pximg.net/a/b.jpg"}]});
        assert_eq!(parse_booth(&b).unwrap().thumb, "https://booth.pximg.net/c/300x300_a2_g5/a/b.jpg");
        assert!(parse_booth(&json!({"is_adult":true})).is_none());
    }

    #[test]
    fn redirect_is_inert() {
        let h = redirect_html("https://www.youtube.com/watch?v=abcdefghijk&t=1");
        assert!(h.contains("url=https://www.youtube.com/watch?v=abcdefghijk&amp;t=1\""));
        assert!(!redirect_html("x\"<script>").contains("<script"));
    }

    #[test]
    fn catalog_meta_is_inert() {
        let it = Item { by: "{@x}<b>".into(), views: 1500, ..Default::default() };
        assert_eq!(catalog_meta("yt", &it), "{\u{200B}@x}<\u{200B}b> · {@feed.views|1.5K}");
    }

    #[test]
    fn weather() {
        let j = json!({"properties":{"timeseries":[{"data":{"instant":{"details":{"air_temperature":30.6}},
            "next_1_hours":{"summary":{"symbol_code":"partlycloudy_day"}}}}]}});
        assert_eq!(weather_line(&j, false).unwrap(), "☁ 31°C");
        assert_eq!(weather_line(&j, true).unwrap(), "☁ 87°F");
        assert_eq!(glyph("clearsky_night"), '★');
    }
}
