//! Copy of the CutePlants app SDK widget builder (Shared/sdk/rust/src/ui.rs), kept here so this repo builds on its own.
//! The world's CPDevice.DrawUi reads what it emits: keep the command format in step with it.
//! Widget tree → the device's own widgets and labels.
//!
//! An app (or cp-server, for feed screens) builds a small Flutter-like tree; `layout` turns it into commands the
//! in-world OS draws with its native layer (rounded rects in theme colours, TMP labels, atlas images, tap areas).
//! Units are design units `u` (docs/DESIGN-SYSTEM.md: short side of the content area / 48), origin at the top-left
//! of the app's content band, y down. The world stretches x to its own width and scrolls y.
//!
//! JSON per command (arrays keep pages small for Udon's VRCJson):
//! - `["r", x, y, w, h, radius, role, alpha]`       rounded rect
//! - `["t", x, y, w, h, size, role, align, lines, text]` label (lines > 1 wraps; text may hold `{@key}` / `{@key|arg}`
//!   which the world translates into the player's language)
//! - `["i", x, y, w, h, cell, radius, badge]`        atlas image cell (badge: small dark tag at the bottom-right, "" = none)
//! - `["a", x, y, w, h, action]`                     tap area; the action string is the app's to interpret

use serde_json::{json, Value};

/// Theme colour roles (CPDevice: Bg, Surface, SurfaceAlt, Ink, InkMuted, Line, Leaf, OnLeaf, Petal, Sun, Sky).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role { Bg = 0, Surface, SurfaceAlt, Ink, InkMuted, Line, Leaf, OnLeaf, Petal, Sun, Sky }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align { Start = 0, Center = 1, End = 2 }

#[derive(Clone, Debug)]
pub enum Node {
    /// Children top to bottom.
    Column { gap: f32, children: Vec<Node> },
    /// Children left to right: `Fixed` children keep their width, the rest share what is left.
    Row { gap: f32, children: Vec<Node> },
    /// `cols` equal columns, rows as tall as their tallest child.
    Grid { cols: usize, gap: f32, children: Vec<Node> },
    /// Text in a box `lines` tall at most (1 = single line with ellipsis). Untrusted text is shown as written.
    Text { text: String, size: f32, role: Role, bold: bool, lines: u32, align: Align, trusted: bool },
    /// Atlas cell at a width / height ratio.
    Image { cell: usize, aspect: f32, radius: f32, badge: String },
    /// Background rect behind a padded child.
    Box { fill: Role, alpha: f32, radius: f32, pad: f32, child: Box<Node> },
    /// Tap area over the child.
    Tap { action: String, child: Box<Node> },
    /// Fixed width (and optional height) for a child.
    Fixed { w: f32, h: Option<f32>, child: Box<Node> },
    Gap(f32),
    /// At least `h` tall (keeps cards in a row lined up when one title is shorter); flexible width.
    Tall { h: f32, child: Box<Node> },
}

// ---------- builders ----------

pub fn col(gap: f32, children: Vec<Node>) -> Node { Node::Column { gap, children } }
pub fn row(gap: f32, children: Vec<Node>) -> Node { Node::Row { gap, children } }
pub fn grid(cols: usize, gap: f32, children: Vec<Node>) -> Node { Node::Grid { cols: cols.max(1), gap, children } }
/// Untrusted text (from a site or a player): rich-text tags are shown literally.
pub fn text(s: &str, size: f32, role: Role, lines: u32) -> Node {
    Node::Text { text: s.into(), size, role, bold: false, lines: lines.max(1), align: Align::Start, trusted: false }
}
/// Our own text: may hold `{@key}` translations.
pub fn label(s: &str, size: f32, role: Role, align: Align) -> Node {
    Node::Text { text: s.into(), size, role, bold: false, lines: 1, align, trusted: true }
}
pub fn bold(n: Node) -> Node { if let Node::Text { text, size, role, lines, align, trusted, .. } = n { Node::Text { text, size, role, bold: true, lines, align, trusted } } else { n } }
pub fn image(cell: usize, aspect: f32, radius: f32, badge: &str) -> Node { Node::Image { cell, aspect, radius, badge: badge.into() } }
pub fn boxed(fill: Role, radius: f32, pad: f32, child: Node) -> Node { Node::Box { fill, alpha: 1.0, radius, pad, child: Box::new(child) } }
pub fn tap(action: &str, child: Node) -> Node { Node::Tap { action: action.into(), child: Box::new(child) } }
pub fn fixed(w: f32, child: Node) -> Node { Node::Fixed { w, h: None, child: Box::new(child) } }
pub fn tall(h: f32, child: Node) -> Node { Node::Tall { h, child: Box::new(child) } }
/// Height of `lines` lines of text at `size`.
pub fn lines_h(lines: u32, size: f32) -> f32 { lines as f32 * size * LINE }
pub fn sized(w: f32, h: f32, child: Node) -> Node { Node::Fixed { w, h: Some(h), child: Box::new(child) } }

// ---------- layout ----------

/// Line height as a multiple of the font size (TMP with the CJK / Thai fallbacks).
const LINE: f32 = 1.3;

/// Rough advance of `s` in units of the font size: wide scripts 1.0, combining marks 0, the rest 0.55.
pub fn text_width(s: &str) -> f32 {
    s.chars().map(|c| {
        let u = c as u32;
        let wide = (0x1100..=0x115F).contains(&u) || (0x2E80..=0xA4CF).contains(&u) || (0xAC00..=0xD7A3).contains(&u)
            || (0xF900..=0xFAFF).contains(&u) || (0xFF00..=0xFF60).contains(&u) || (0x1F300..=0x1FAFF).contains(&u);
        let mark = (0x0300..=0x036F).contains(&u) || u == 0x0E31 || (0x0E34..=0x0E3A).contains(&u) || (0x0E47..=0x0E4E).contains(&u) || u == 0x200B;
        if mark { 0.0 } else if wide { 1.0 } else { 0.55 }
    }).sum()
}

fn text_lines(s: &str, size: f32, w: f32, max: u32) -> u32 {
    if max <= 1 || w <= 0.0 { return 1; }
    let n: u32 = s.split('\n').map(|p| ((text_width(p) * size / w).ceil() as u32).max(1)).sum();
    n.clamp(1, max)
}

/// Shows untrusted text literally: a zero-width space after every `<` means it can never open a TMP tag
/// (`<noparse>` would also hide the tags the device adds for Thai / Arabic shaping), and `{@` is not a translation.
/// Makes untrusted text inert (no rich-text tags, no `{@key}` lookups) so it can sit inside a trusted label.
pub fn literal(s: &str) -> String {
    s.replace('<', "<\u{200B}").replace("{@", "{\u{200B}@")
}

impl Node {
    /// Height when laid out `w` wide.
    pub fn height(&self, w: f32) -> f32 {
        match self {
            Node::Column { gap, children } => children.iter().map(|c| c.height(w)).sum::<f32>() + gap * children.len().saturating_sub(1) as f32,
            Node::Row { gap, children } => row_widths(children, *gap, w).iter().zip(children).map(|(cw, c)| c.height(*cw)).fold(0.0, f32::max),
            Node::Grid { cols, gap, children } => {
                let cw = (w - gap * (*cols as f32 - 1.0)) / *cols as f32;
                let rows: Vec<f32> = children.chunks(*cols).map(|r| r.iter().map(|c| c.height(cw)).fold(0.0, f32::max)).collect();
                rows.iter().sum::<f32>() + gap * rows.len().saturating_sub(1) as f32
            }
            Node::Text { text, size, lines, .. } => text_lines(text, *size, w, *lines) as f32 * size * LINE,
            Node::Image { aspect, .. } => w / aspect.max(0.01),
            Node::Box { pad, child, .. } => child.height(w - 2.0 * pad) + 2.0 * pad,
            Node::Tap { child, .. } => child.height(w),
            Node::Fixed { w: fw, h, child } => h.unwrap_or_else(|| child.height(fw.min(w))),
            Node::Gap(h) => *h,
            Node::Tall { h, child } => child.height(w).max(*h),
        }
    }

    /// Emits commands for this node at (x, y), `w` wide.
    pub fn emit(&self, x: f32, y: f32, w: f32, out: &mut Vec<Value>) {
        let r = |v: f32| (v as f64 * 100.0).round() / 100.0; // 0.01 u is plenty and keeps the JSON short
        match self {
            Node::Column { gap, children } => {
                let mut cy = y;
                for c in children { c.emit(x, cy, w, out); cy += c.height(w) + gap; }
            }
            Node::Row { gap, children } => {
                let mut cx = x;
                for (c, cw) in children.iter().zip(row_widths(children, *gap, w)) { c.emit(cx, y, cw, out); cx += cw + gap; }
            }
            Node::Grid { cols, gap, children } => {
                let cw = (w - gap * (*cols as f32 - 1.0)) / *cols as f32;
                let mut cy = y;
                for rowc in children.chunks(*cols) {
                    let h = rowc.iter().map(|c| c.height(cw)).fold(0.0, f32::max);
                    for (k, c) in rowc.iter().enumerate() { c.emit(x + k as f32 * (cw + gap), cy, cw, out); }
                    cy += h + gap;
                }
            }
            Node::Text { text, size, role, bold, lines, align, trusted } => {
                let n = text_lines(text, *size, w, *lines);
                let body = if *trusted { text.clone() } else { literal(text) };
                let body = if *bold { format!("<b>{body}</b>") } else { body };
                out.push(json!(["t", r(x), r(y), r(w), r(n as f32 * size * LINE), r(*size), *role as u8, *align as u8, n, body]));
            }
            Node::Image { cell, aspect, radius, badge } => {
                out.push(json!(["i", r(x), r(y), r(w), r(w / aspect.max(0.01)), cell, r(*radius), badge]));
            }
            Node::Box { fill, alpha, radius, pad, child } => {
                out.push(json!(["r", r(x), r(y), r(w), r(self.height(w)), r(*radius), *fill as u8, alpha]));
                child.emit(x + pad, y + pad, w - 2.0 * pad, out);
            }
            Node::Tap { action, child } => {
                out.push(json!(["a", r(x), r(y), r(w), r(child.height(w)), action]));
                child.emit(x, y, w, out);
            }
            Node::Fixed { w: fw, child, .. } => child.emit(x, y, fw.min(w), out),
            Node::Gap(_) => {}
            Node::Tall { child, .. } => child.emit(x, y, w, out),
        }
    }
}

fn row_widths(children: &[Node], gap: f32, w: f32) -> Vec<f32> {
    let fixed: f32 = children.iter().filter_map(|c| if let Node::Fixed { w, .. } = c { Some(*w) } else { None }).sum();
    let flex = children.iter().filter(|c| !matches!(c, Node::Fixed { .. })).count();
    let rest = (w - fixed - gap * children.len().saturating_sub(1) as f32).max(0.0);
    children.iter().map(|c| if let Node::Fixed { w, .. } = c { *w } else { rest / flex.max(1) as f32 }).collect()
}

/// One screen: `{"h": content height, "c": [commands]}` for a band `w` units wide.
pub fn view(root: &Node, w: f32) -> Value {
    let mut c = Vec::new();
    root.emit(0.0, 0.0, w, &mut c);
    json!({ "h": (root.height(w) as f64 * 100.0).round() / 100.0, "c": c })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_layout() {
        // thumbnail 16 wide on the left, text column takes the rest
        let n = tap("v:1", row(1.0, vec![fixed(16.0, image(3, 16.0 / 9.0, 1.0, "3:12")), col(0.5, vec![text("Hello <b>x</b>", 1.5, Role::Ink, 2), label("{@feed.views|1K}", 1.2, Role::InkMuted, Align::Start)])]));
        let v = view(&n, 48.0);
        let c = v["c"].as_array().unwrap();
        assert_eq!(c[0], json!(["a", 0.0, 0.0, 48.0, 9.0, "v:1"]));
        assert_eq!(c[1], json!(["i", 0.0, 0.0, 16.0, 9.0, 3, 1.0, "3:12"]));
        assert_eq!(c[2][0], "t");
        assert_eq!(c[2][1], 17.0); // 16 + gap 1
        assert_eq!(c[2][3], 31.0); // 48 - 16 - 1
        assert_eq!(c[2][9], "Hello <\u{200B}b>x<\u{200B}/b>"); // site text never becomes rich text
        assert_eq!(c[3][9], "{@feed.views|1K}");
        assert_eq!(v["h"], 9.0);
    }

    #[test]
    fn wrapping_estimate() {
        assert_eq!(text_lines("short", 1.5, 30.0, 3), 1);
        assert_eq!(text_lines(&"word ".repeat(40), 1.5, 30.0, 3), 3); // capped
        assert!(text_width("日本語") > text_width("abc"));
        assert_eq!(text_width("\u{0E01}\u{0E34}"), 0.55); // Thai mark takes no width
        assert_eq!(literal("{@feed.x} <i>"), "{\u{200B}@feed.x} <\u{200B}i>");
    }

    #[test]
    fn grid_rows() {
        let cards: Vec<Node> = (0..5).map(|i| image(i, 1.0, 0.0, "")).collect();
        let v = view(&grid(2, 2.0, cards), 42.0); // cells 20 wide
        assert_eq!(v["h"], 64.0); // 3 rows of 20 + 2 gaps
        assert_eq!(v["c"][3][1], 22.0);
        assert_eq!(v["c"][3][2], 22.0);
    }
}
