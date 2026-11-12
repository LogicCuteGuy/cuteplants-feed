//! Feed screens built with the app SDK (`ui.rs`): the world draws them with its own widgets and labels.
//! Responsive: each page carries one layout per design width in `WIDTHS` (u = short side / 48, so a portrait phone or
//! a watch is 48 wide, 4:3 about 64, 16:9 about 85, ultra-wide about 112). Column counts and the detail arrangement
//! come from the width; the world picks the widest layout that fits and stretches it the last few units.
//! Each layout is a list view plus one detail view per item. Tap actions: `v:k` open view k, `b` back to the list,
//! `p:n` play item n's video, `o:n` open item n's image (n = index on this page).

use crate::ui::*;
use serde_json::{json, Map, Value};

use crate::Item;

const WIDTHS: [f32; 4] = [48.0, 64.0, 84.0, 112.0];
const PAD: f32 = 1.0;
// type scale and radii in u, as in the world's DESIGN-SYSTEM.md: nothing smaller than T_MIN
const T_LIST: f32 = 2.4;
const T_BODY: f32 = 2.1;
const T_BUTTON: f32 = 2.2;
const T_SUB: f32 = 1.9;
const T_MIN: f32 = 1.7;
const R_GRID: f32 = 2.7; // thumbnails in a grid, big pictures
const R_LIST: f32 = 1.6; // thumbnails in a list row
const R_PILL: f32 = 9.0; // clamped to half the height: a pill

/// `{"48": {...}, "64": {...}, ...}`, each `{"w": design width, "pad", "views": [list, detail 1, …]}`.
pub fn screens(src: &str, items: &[Item]) -> Value {
    let mut m = Map::new();
    for w in WIDTHS { m.insert(format!("{w}"), layout(src, items, w)); }
    Value::Object(m)
}

fn layout(src: &str, items: &[Item], w: f32) -> Value {
    let inner = w - 2.0 * PAD;
    let mut views = vec![view(&list(src, items, inner), inner)];
    for (i, it) in items.iter().enumerate() { views.push(view(&detail(src, i, it, inner), inner)); }
    json!({ "w": w, "pad": PAD, "views": views })
}

pub fn compact_num(v: i64) -> String {
    let f = v as f64;
    let one = |x: f64, s: &str| { let t = format!("{x:.1}"); format!("{}{s}", t.trim_end_matches(".0")) };
    if f >= 1e9 { one(f / 1e9, "B") } else if f >= 1e6 { one(f / 1e6, "M") } else if f >= 1e3 { one(f / 1e3, "K") } else { v.to_string() }
}

pub fn length(secs: i64) -> String {
    if secs <= 0 { return String::new(); }
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Columns of at least `min` units in `w` (gap 1.5).
fn cols(w: f32, min: f32) -> usize { (((w + 1.5) / (min + 1.5)).floor() as usize).max(1) }

/// "Channel · 1.2K views" / "Channel · ● Live" on one line; BOOTH: "¥500  ♡ 1.2K". `{@key|arg}` is translated in the world.
fn meta(src: &str, it: &Item, size: f32) -> Option<Node> {
    if src == "booth" {
        let wish = if it.views >= 0 { format!("  ♡ {}", compact_num(it.views)) } else { String::new() };
        if it.price.is_empty() && wish.is_empty() { return None; }
        return Some(label(&format!("<b>{}</b>{wish}", literal(&it.price)), size, Role::Ink, Align::Start));
    }
    let by = literal(&it.by);
    let sep = if by.is_empty() { "" } else { " · " };
    if it.live {
        let watching = if it.views >= 0 { format!(" {{@feed.watching|{}}}", compact_num(it.views)) } else { String::new() };
        return Some(label(&format!("{by}{sep}● {{@feed.live}}{watching}"), size, Role::Petal, Align::Start));
    }
    let views = if it.views >= 0 { format!("{{@feed.views|{}}}", compact_num(it.views)) } else { String::new() };
    let s = if views.is_empty() { by } else { format!("{by}{sep}{views}") };
    (!s.is_empty()).then(|| label(&s, size, Role::InkMuted, Align::Start))
}

fn badge(it: &Item) -> String { if it.live { String::new() } else { length(it.dur) } }

/// Text that always takes `lines` lines, so cards side by side line up.
fn block(s: &str, size: f32, role: Role, lines: u32) -> Node { tall(lines_h(lines, size), text(s, size, role, lines)) }

fn rule() -> Node { boxed(Role::Line, 0.0, 0.0, Node::Gap(0.12)) }

fn list(src: &str, items: &[Item], w: f32) -> Node {
    let (n, min) = match src { "booth" => (0, 20.0), "x" => (1, 40.0), _ => (2, 30.0) };
    let c = cols(w, min);
    let cards: Vec<Node> = items.iter().enumerate().map(|(i, it)| {
        let open = format!("v:{}", i + 1);
        match n {
            0 => tap(&open, col(0.5, [
                vec![image(i, 1.0, R_GRID, ""), block(&it.title, T_SUB, Role::Ink, 2), text(&it.by, T_MIN, Role::InkMuted, 1)],
                meta(src, it, T_MIN).into_iter().collect(),
            ].concat())),
            1 => {
                let mut name = vec![bold(text(&it.by, T_SUB, Role::Ink, 1))];
                if !it.video.is_empty() { name.push(fixed(2.5, label("▶", T_MIN, Role::InkMuted, Align::End))); }
                tap(&open, col(1.2, vec![
                    row(1.0, vec![fixed(8.0, image(i, 1.0, R_LIST, "")), col(0.3, vec![row(0.5, name), text(&it.title, T_SUB, Role::Ink, 4)])]),
                    rule(),
                ]))
            }
            _ => {
                let thumb = if w < 40.0 { 16.0 } else { 15.0 };
                tap(&open, row(1.0, vec![
                    fixed(thumb, image(i, 16.0 / 9.0, R_LIST, &badge(it))),
                    col(0.4, [vec![block(&it.title, T_SUB, Role::Ink, 2)], meta(src, it, T_MIN).into_iter().collect()].concat()),
                ]))
            }
        }
    }).collect();
    grid(c, if n == 1 { 1.2 } else { 1.5 }, cards)
}

fn detail(src: &str, i: usize, it: &Item, w: f32) -> Node {
    let side = w > 60.0; // picture beside the text from 4:3 landscape up
    let back = tap("b", fixed(12.0, boxed(Role::Surface, R_PILL, 0.7, label("‹ {@feed.back}", T_BUTTON, Role::Ink, Align::Center))));
    let aspect = if src == "yt" { 16.0 / 9.0 } else { 1.0 };
    let mut body = if src == "x" {
        vec![bold(text(&it.by, T_BODY, Role::Ink, 1)), text(&it.title, T_BODY, Role::Ink, 14)]
    } else {
        let mut v = vec![bold(text(&it.title, T_LIST, Role::Ink, 3))];
        if src == "booth" { v.push(text(&it.by, T_SUB, Role::InkMuted, 1)); }
        v.extend(meta(src, it, T_SUB));
        v
    };
    let action = if !it.video.is_empty() { Some(("p", "{@feed.watch}")) } else if !it.image.is_empty() { Some(("o", "{@feed.open}")) } else { None };
    if let Some((verb, name)) = action {
        body.push(Node::Gap(0.6));
        body.push(tap(&format!("{verb}:{i}"), fixed(16.0, boxed(Role::Leaf, R_PILL, 0.8, bold(label(name, T_BUTTON, Role::OnLeaf, Align::Center))))));
    }
    // the picture never takes more than ~28 u of height (the band is ~36 u on a landscape screen)
    let main = if side {
        let hero_w = (w * 0.45).min(28.0 * aspect);
        row(2.0, vec![fixed(hero_w, image(i, aspect, R_GRID, &badge(it))), col(0.8, body)])
    } else {
        let hero_w = w.min(30.0 * aspect);
        let hero = fixed(hero_w, image(i, aspect, R_GRID, &badge(it)));
        let hero = if hero_w < w { row(0.0, vec![Node::Gap(0.0), hero, Node::Gap(0.0)]) } else { hero }; // centred
        col(1.2, [vec![hero], body].concat())
    };
    col(1.2, vec![back, main])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(t: &str) -> Item { Item { title: t.into(), by: "ch".into(), views: 1234, dur: 192, video: "v".into(), ..Default::default() } }

    #[test]
    fn numbers() {
        assert_eq!(compact_num(999), "999");
        assert_eq!(compact_num(1234), "1.2K");
        assert_eq!(compact_num(2_000_000), "2M");
        assert_eq!(length(192), "3:12");
        assert_eq!(length(3725), "1:02:05");
        assert_eq!(length(0), "");
    }

    #[test]
    fn responsive_columns() {
        // inner widths of the 48 / 64 / 84 / 112 layouts
        assert_eq!([46.0, 62.0, 82.0, 110.0].map(|w| cols(w, 20.0)), [2, 2, 3, 5]);
        assert_eq!([46.0, 62.0, 82.0, 110.0].map(|w| cols(w, 30.0)), [1, 2, 2, 3]);
    }

    #[test]
    fn views_and_actions() {
        let s = screens("yt", &[item("a"), item("b")]);
        let c = &s["48"];
        assert_eq!(c["w"], 48.0);
        assert_eq!(c["views"].as_array().unwrap().len(), 3); // list + 2 details
        let list = c["views"][0]["c"].as_array().unwrap();
        assert_eq!(list[0][5], "v:1");
        assert!(list.iter().any(|x| x[0] == "i" && x[7] == "3:12"));
        assert!(list.iter().any(|x| x[0] == "t" && x[9] == "ch · {@feed.views|1.2K}"));
        let detail = c["views"][2]["c"].as_array().unwrap();
        assert_eq!(detail[0][5], "b");
        assert!(detail.iter().any(|x| x[0] == "a" && x[5] == "p:1"));
        for w in ["64", "84", "112"] { assert_eq!(s[w]["views"].as_array().unwrap().len(), 3); }
        // booth item without a video opens its image; text-only X post has no action
        let b = Item { image: "img".into(), ..item("c") };
        assert!(screens("booth", &[Item { video: String::new(), ..b }])["48"]["views"][1]["c"].as_array().unwrap().iter().any(|x| x[5] == "o:0"));
        let t = Item { video: String::new(), ..item("d") };
        assert!(!screens("x", &[t])["48"]["views"][1]["c"].as_array().unwrap().iter().any(|x| x[0] == "a" && x[5] != "b"));
        // a channel name can't inject a lookup or a tag into the trusted meta line
        let evil = Item { by: "{@x}<b>".into(), ..item("e") };
        let l = &screens("yt", &[evil])["48"]["views"][0]["c"];
        assert!(l.as_array().unwrap().iter().any(|x| x[0] == "t" && x[9].as_str().unwrap().starts_with("{\u{200B}@x}<\u{200B}b>")));
    }
}
