//! Weather: one city per region from Open-Meteo (free, no key, local times). Writes the home line `wx/{CC}.txt`
//! ("☀ 31°C\tBangkok") and the Weather app's screens `wx/{CC}.json` ({"city", "ui"}): now, next 24 hours, 7 days.
//! Words are string keys (`{@wx.rain}`), so every player reads them in their own language; glyphs are the ones the
//! world's font chain has (☀ ☁ ☂ ☃ ★).

use std::{fs, path::Path};

use serde_json::{json, Map, Value};

use crate::{run, ui::*, REGIONS};

const WIDTHS: [f32; 4] = [48.0, 64.0, 84.0, 112.0];
const PAD: f32 = 1.0;

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

pub fn weather_all(out: &Path) {
    for cc in REGIONS {
        let (lat, lon, city) = region_city(cc);
        let us = cc == "US";
        let units = if us { "&temperature_unit=fahrenheit&wind_speed_unit=mph" } else { "" };
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lat:.2}&longitude={lon:.2}\
             &current=temperature_2m,apparent_temperature,relative_humidity_2m,wind_speed_10m,weather_code,is_day\
             &hourly=temperature_2m,weather_code,is_day,precipitation_probability\
             &daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max\
             &timezone=auto&forecast_days=7{units}"
        );
        let got = run("curl", &["-sSfL", "--max-time", "20", "--retry", "2", &url]).and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string()));
        match got.ok().and_then(|j| Forecast::read(&j, us)) {
            Some(f) => {
                fs::write(out.join(format!("wx/{cc}.txt")), format!("{} {}\t{city}", f.glyph, f.temp)).ok();
                fs::write(out.join(format!("wx/{cc}.json")), json!({ "city": city, "ui": screens(&f) }).to_string()).ok();
            }
            None => eprintln!("weather {cc}: no forecast"),
        }
    }
}

/// WMO weather code → (glyph, condition key). Night turns a clear sky into a star.
pub fn condition(code: i64, day: bool) -> (char, &'static str) {
    match code {
        0 => (if day { '☀' } else { '★' }, "wx.clear"),
        1 => (if day { '☀' } else { '★' }, "wx.mostlyClear"),
        2 => ('☁', "wx.partlyCloudy"),
        3 => ('☁', "wx.cloudy"),
        45 | 48 => ('☁', "wx.fog"),
        51..=57 => ('☂', "wx.drizzle"),
        61..=67 => ('☂', "wx.rain"),
        80..=82 => ('☂', "wx.showers"),
        95..=99 => ('☂', "wx.thunder"),
        71..=77 | 85 | 86 => ('☃', "wx.snow"),
        _ => ('☁', "wx.cloudy"),
    }
}

pub struct Hour { pub time: String, pub glyph: char, pub temp: String, pub rain: i64 }
pub struct Day { pub weekday: usize, pub glyph: char, pub hi: String, pub lo: String, pub rain: i64 }

pub struct Forecast {
    pub glyph: char,
    pub cond: &'static str,
    pub temp: String,
    pub feels: String,
    pub humidity: i64,
    pub wind: String,
    pub hours: Vec<Hour>, // every 3 h from now, 8 of them
    pub days: Vec<Day>,
}

fn deg(t: f64, f: bool) -> String { format!("{}°{}", t.round() as i64, if f { "F" } else { "C" }) }
fn short(t: f64) -> String { format!("{}°", t.round() as i64) }

/// Day of the week of a "YYYY-MM-DD" date, 0 = Monday (Sakamoto).
pub fn weekday(date: &str) -> Option<usize> {
    let mut p = date.split('-').map(|x| x.parse::<i64>().ok());
    let (mut y, m, d) = (p.next()??, p.next()??, p.next()??);
    const T: [i64; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if m < 3 { y -= 1; }
    let sun0 = (y + y / 4 - y / 100 + y / 400 + T[(m - 1).clamp(0, 11) as usize] + d) % 7; // 0 = Sunday
    Some(((sun0 + 6) % 7) as usize)
}

impl Forecast {
    pub fn read(j: &Value, fahrenheit: bool) -> Option<Forecast> {
        let c = &j["current"];
        let (glyph, cond) = condition(c["weather_code"].as_i64()?, c["is_day"].as_i64() != Some(0));
        let h = &j["hourly"];
        let now = c["time"].as_str()?.get(..13)?; // "2026-11-09T09"
        let times = h["time"].as_array()?;
        let start = times.iter().position(|t| t.as_str().and_then(|s| s.get(..13)) == Some(now)).unwrap_or(0);
        let hours = (0..8).filter_map(|k| {
            let i = start + k * 3;
            let t = times.get(i)?.as_str()?;
            let (g, _) = condition(h["weather_code"][i].as_i64()?, h["is_day"][i].as_i64() != Some(0));
            Some(Hour { time: t.get(11..16)?.to_owned(), glyph: g, temp: short(h["temperature_2m"][i].as_f64()?), rain: h["precipitation_probability"][i].as_i64().unwrap_or(-1) })
        }).collect();
        let d = &j["daily"];
        let days = d["time"].as_array()?.iter().enumerate().filter_map(|(i, t)| {
            let (g, _) = condition(d["weather_code"][i].as_i64()?, true);
            Some(Day {
                weekday: weekday(t.as_str()?)?,
                glyph: g,
                hi: short(d["temperature_2m_max"][i].as_f64()?),
                lo: short(d["temperature_2m_min"][i].as_f64()?),
                rain: d["precipitation_probability_max"][i].as_i64().unwrap_or(-1),
            })
        }).collect();
        Some(Forecast {
            glyph, cond,
            temp: deg(c["temperature_2m"].as_f64()?, fahrenheit),
            feels: short(c["apparent_temperature"].as_f64()?),
            humidity: c["relative_humidity_2m"].as_i64().or(c["relative_humidity_2m"].as_f64().map(|x| x as i64)).unwrap_or(-1),
            wind: format!("{} {}", c["wind_speed_10m"].as_f64().unwrap_or(0.0).round() as i64, if fahrenheit { "mph" } else { "km/h" }),
            hours, days,
        })
    }
}

// ---------- screens (one view per design width, same format as the feed pages) ----------

const WEEK: [&str; 7] = ["wx.mon", "wx.tue", "wx.wed", "wx.thu", "wx.fri", "wx.sat", "wx.sun"];

pub fn screens(f: &Forecast) -> Value {
    let mut m = Map::new();
    for w in WIDTHS {
        let inner = w - 2.0 * PAD;
        m.insert(format!("{w}"), json!({ "w": w, "pad": PAD, "views": [view(&body(f, inner), inner)] }));
    }
    Value::Object(m)
}

fn card(child: Node) -> Node { boxed(Role::Surface, 1.4, 1.2, child) }
fn center(s: &str, size: f32, role: Role) -> Node { label(s, size, role, Align::Center) }
fn rain(p: i64) -> String { if p > 0 { format!("☂ {p}%") } else { String::new() } }

fn body(f: &Forecast, w: f32) -> Node {
    let hero = card(row(1.5, vec![
        fixed(8.0, center(&f.glyph.to_string(), 6.0, Role::Ink)),
        col(0.4, vec![
            bold(label(&f.temp, 4.2, Role::Ink, Align::Start)),
            label(&format!("{{@{}}}", f.cond), 1.7, Role::Ink, Align::Start),
            label(&match f.days.first() { Some(d) => format!("{} / {}  ·  {{@wx.feels|{}}}", d.hi, d.lo, f.feels), None => format!("{{@wx.feels|{}}}", f.feels) }, 1.3, Role::InkMuted, Align::Start),
        ]),
    ]));
    let today_rain = f.days.first().map_or(-1, |d| d.rain);
    let stat = |key: &str, v: String| card(col(0.3, vec![label(&format!("{{@{key}}}"), 1.2, Role::InkMuted, Align::Start), bold(label(&v, 1.6, Role::Ink, Align::Start))]));
    let stats = grid(3, 1.0, vec![
        stat("wx.humidity", if f.humidity >= 0 { format!("{}%", f.humidity) } else { "–".into() }),
        stat("wx.wind", f.wind.clone()),
        stat("wx.rainChance", if today_rain >= 0 { format!("{today_rain}%") } else { "–".into() }),
    ]);
    let hour_cols = if w >= 60.0 { 8 } else { 4 };
    let hours = card(col(0.8, vec![
        label("{@wx.hourly}", 1.3, Role::InkMuted, Align::Start),
        grid(hour_cols, 0.6, f.hours.iter().enumerate().map(|(k, h)| col(0.2, vec![
            center(if k == 0 { "{@wx.now}" } else { &h.time }, 1.15, Role::InkMuted),
            center(&h.glyph.to_string(), 2.2, Role::Ink),
            bold(center(&h.temp, 1.4, Role::Ink)),
            center(&rain(h.rain), 1.0, Role::Sky),
        ])).collect()),
    ]));
    let mut week = vec![label("{@wx.week}", 1.3, Role::InkMuted, Align::Start)];
    for (k, d) in f.days.iter().enumerate() {
        if k > 0 { week.push(boxed(Role::Line, 0.0, 0.0, Node::Gap(0.12))); }
        let name = if k == 0 { "{@wx.today}".to_owned() } else { format!("{{@{}}}", WEEK[d.weekday]) };
        week.push(row(1.0, vec![
            fixed(11.0, label(&name, 1.4, Role::Ink, Align::Start)),
            fixed(4.0, center(&d.glyph.to_string(), 1.8, Role::Ink)),
            label(&rain(d.rain), 1.2, Role::Sky, Align::Start),
            fixed(12.0, label(&format!("<b>{}</b>  {}", d.hi, d.lo), 1.4, Role::Ink, Align::End)),
        ]));
    }
    let week = card(col(0.7, week));
    let note = label("Open-Meteo.com (CC BY 4.0)", 1.0, Role::InkMuted, Align::End);
    if w > 70.0 {
        // wide: now + stats on the left, the week on the right
        col(1.2, vec![row(1.2, vec![col(1.2, vec![hero, stats, hours]), fixed((w * 0.4).min(44.0), week)]), note])
    } else {
        col(1.2, vec![hero, stats, hours, week, note])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_codes() {
        assert_eq!(weekday("2026-11-09"), Some(0)); // a Monday
        assert_eq!(weekday("2024-02-29"), Some(3)); // Thursday
        assert_eq!(condition(0, false).0, '★');
        assert_eq!(condition(81, true), ('☂', "wx.showers"));
        assert_eq!(condition(73, true).0, '☃');
    }

    #[test]
    fn reads_open_meteo() {
        let j = json!({
            "current": {"time":"2026-11-09T09:15","temperature_2m":29.6,"apparent_temperature":33.9,"relative_humidity_2m":65,"wind_speed_10m":5.5,"weather_code":3,"is_day":1},
            "hourly": {"time":["2026-11-09T08:00","2026-11-09T09:00","2026-11-09T10:00","2026-11-09T11:00","2026-11-09T12:00"],
                "temperature_2m":[28.0,29.6,30.2,31.0,32.4],"weather_code":[2,3,3,61,61],"is_day":[1,1,1,1,1],"precipitation_probability":[0,5,10,40,60]},
            "daily": {"time":["2026-11-09","2026-11-10"],"weather_code":[53,3],"temperature_2m_max":[33.6,31.8],"temperature_2m_min":[25.1,25.3],"precipitation_probability_max":[75,16]}
        });
        let f = Forecast::read(&j, false).unwrap();
        assert_eq!((f.glyph, f.cond, f.temp.as_str(), f.feels.as_str(), f.humidity), ('☁', "wx.cloudy", "30°C", "34°", 65));
        assert_eq!(f.hours.len(), 2); // 09:00 and 12:00 (3 h steps from now)
        assert_eq!((f.hours[1].time.as_str(), f.hours[1].glyph, f.hours[1].rain), ("12:00", '☂', 60));
        assert_eq!((f.days[0].weekday, f.days[0].hi.as_str(), f.days[1].lo.as_str()), (0, "34°", "25°"));
        let s = screens(&f);
        for w in ["48", "64", "84", "112"] {
            let c = s[w]["views"][0]["c"].as_array().unwrap();
            assert!(c.iter().any(|x| x[0] == "t" && x[9] == "<b>30°C</b>"));
            assert!(c.iter().any(|x| x[0] == "t" && x[9] == "{@wx.today}"));
        }
        assert!(Forecast::read(&json!({}), false).is_none());
    }
}
