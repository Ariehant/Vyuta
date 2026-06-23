//! Hand-rolled PX4 **ULog** parser (no `nom`).
//!
//! Reads the ULog binary format — header, definitions ('F' formats, 'A'
//! subscriptions, 'I' info) and data ('D' records, 'L' logged strings) — and
//! decodes every flat numeric field of every subscribed message into a named
//! time series (`message[instance].field`, arrays expanded to `field[i]`).
//! Nested-struct fields are size-skipped (the common review topics are flat).
//!
//! Spec: <https://docs.px4.io/main/en/dev_log/ulog_file_format.html>

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Result};

use crate::model::{Log, LogMessage, ModeSpan, Series};

const MAGIC: [u8; 7] = [0x55, 0x4C, 0x6F, 0x67, 0x01, 0x12, 0x35];
const MAX_ARRAY_EXPAND: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Base {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    Bool,
    Char,
}

impl Base {
    fn from_str(s: &str) -> Option<Base> {
        Some(match s {
            "uint8_t" => Base::U8,
            "int8_t" => Base::I8,
            "uint16_t" => Base::U16,
            "int16_t" => Base::I16,
            "uint32_t" => Base::U32,
            "int32_t" => Base::I32,
            "uint64_t" => Base::U64,
            "int64_t" => Base::I64,
            "float" => Base::F32,
            "double" => Base::F64,
            "bool" => Base::Bool,
            "char" => Base::Char,
            _ => return None,
        })
    }

    fn size(self) -> usize {
        match self {
            Base::U8 | Base::I8 | Base::Bool | Base::Char => 1,
            Base::U16 | Base::I16 => 2,
            Base::U32 | Base::I32 | Base::F32 => 4,
            Base::U64 | Base::I64 | Base::F64 => 8,
        }
    }

    /// Read one value as f64; returns None for `char` (treated as text).
    fn read(self, b: &[u8]) -> Option<f64> {
        Some(match self {
            Base::U8 => b[0] as f64,
            Base::I8 => (b[0] as i8) as f64,
            Base::Bool => (b[0] != 0) as u8 as f64,
            Base::U16 => u16::from_le_bytes([b[0], b[1]]) as f64,
            Base::I16 => i16::from_le_bytes([b[0], b[1]]) as f64,
            Base::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Base::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Base::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Base::U64 => u64::from_le_bytes(b[0..8].try_into().ok()?) as f64,
            Base::I64 => i64::from_le_bytes(b[0..8].try_into().ok()?) as f64,
            Base::F64 => f64::from_le_bytes(b[0..8].try_into().ok()?),
            Base::Char => return None,
        })
    }
}

/// A raw field token from a format string: `(type_token, name)`.
type RawField = (String, String);

struct ResolvedField {
    name: String,
    base: Option<Base>, // None => nested struct (skipped)
    array_len: usize,
    total_size: usize,
}

struct Subscription {
    message: String,
    multi_id: u8,
}

/// Parse a complete ULog buffer.
pub fn parse(bytes: &[u8], source: &str, name: &str) -> Result<Log> {
    if bytes.len() < 16 || bytes[0..7] != MAGIC {
        bail!("not a ULog file (bad magic)");
    }
    let mut cur = Cursor::new(bytes);
    cur.pos = 16; // skip 7 magic + 1 version + 8 timestamp

    let mut formats: BTreeMap<String, Vec<RawField>> = BTreeMap::new();
    let mut resolved: BTreeMap<String, Vec<ResolvedField>> = BTreeMap::new();
    let mut subs: BTreeMap<u16, Subscription> = BTreeMap::new();
    let mut info: BTreeMap<String, String> = BTreeMap::new();

    let mut series: BTreeMap<String, Series> = BTreeMap::new();
    let mut messages: Vec<LogMessage> = Vec::new();
    let mut min_ts: Option<u64> = None;

    while let Some((mtype, payload)) = cur.next_message() {
        match mtype {
            b'F' => {
                if let Some((mname, fields)) = parse_format(payload) {
                    formats.insert(mname, fields);
                }
            }
            b'I' => {
                if let Some((k, v)) = parse_info(payload) {
                    info.insert(k, v);
                }
            }
            b'A' => {
                if payload.len() >= 3 {
                    let multi_id = payload[0];
                    let msg_id = u16::from_le_bytes([payload[1], payload[2]]);
                    let message = String::from_utf8_lossy(&payload[3..]).into_owned();
                    subs.insert(msg_id, Subscription { message, multi_id });
                }
            }
            b'D' => {
                if payload.len() < 2 {
                    continue;
                }
                let msg_id = u16::from_le_bytes([payload[0], payload[1]]);
                let data = &payload[2..];
                let Some(sub) = subs.get(&msg_id) else {
                    continue;
                };
                let layout = resolve(&sub.message, &formats, &mut resolved);
                if let Some(fields) = layout {
                    decode_record(
                        &sub.message,
                        sub.multi_id,
                        fields,
                        data,
                        &mut series,
                        &mut min_ts,
                    );
                }
            }
            b'L' => {
                if payload.len() >= 9 {
                    let level = payload[0];
                    let ts = u64::from_le_bytes(payload[1..9].try_into().unwrap());
                    let text = String::from_utf8_lossy(&payload[9..]).into_owned();
                    track_min(&mut min_ts, ts);
                    messages.push(LogMessage {
                        t: ts as f64,
                        level,
                        text,
                    });
                }
            }
            // 'B' flags, 'P'/'Q' params, 'M' multi-info, 'R' unsub, 'S' sync,
            // 'O' dropout, 'C' tagged string — not needed for the browser.
            _ => {}
        }
    }

    // Normalise timestamps (raw microseconds) to seconds since the first sample.
    let base = min_ts.unwrap_or(0) as f64;
    let mut duration = 0.0_f64;
    for s in series.values_mut() {
        for t in &mut s.t {
            *t = (*t - base) / 1e6;
            duration = duration.max(*t);
        }
    }
    for m in &mut messages {
        m.t = (m.t - base) / 1e6;
    }

    let modes = build_modes(&series);

    Ok(Log {
        source: source.to_string(),
        name: name.to_string(),
        series,
        modes,
        messages,
        info,
        duration_s: duration,
    })
}

fn decode_record(
    message: &str,
    multi_id: u8,
    fields: &[ResolvedField],
    data: &[u8],
    series: &mut BTreeMap<String, Series>,
    min_ts: &mut Option<u64>,
) {
    // First pass: locate the record timestamp (uint64 `timestamp`).
    let mut off = 0usize;
    let mut t_us: Option<u64> = None;
    let mut pending: Vec<(String, f64)> = Vec::new();

    for f in fields {
        if off + f.total_size > data.len() {
            return; // truncated record
        }
        let bytes = &data[off..off + f.total_size];
        if f.name == "timestamp" && f.base == Some(Base::U64) && f.array_len == 1 {
            t_us = Some(u64::from_le_bytes(bytes[0..8].try_into().unwrap()));
        } else if f.name.starts_with("_padding") {
            // skip
        } else if let Some(base) = f.base {
            if base != Base::Char {
                let esize = base.size();
                if f.array_len == 1 {
                    if let Some(v) = base.read(bytes) {
                        pending.push((format!("{message}[{multi_id}].{}", f.name), v));
                    }
                } else if f.array_len <= MAX_ARRAY_EXPAND {
                    for i in 0..f.array_len {
                        let chunk = &bytes[i * esize..(i + 1) * esize];
                        if let Some(v) = base.read(chunk) {
                            pending.push((format!("{message}[{multi_id}].{}[{i}]", f.name), v));
                        }
                    }
                }
            }
        }
        off += f.total_size;
    }

    let Some(ts) = t_us else {
        return;
    };
    track_min(min_ts, ts);
    let t = ts as f64;
    for (key, v) in pending {
        series.entry(key).or_default().push(t, v);
    }
}

/// Resolve (and cache) a message's field layout, computing field sizes.
fn resolve<'a>(
    name: &str,
    formats: &BTreeMap<String, Vec<RawField>>,
    cache: &'a mut BTreeMap<String, Vec<ResolvedField>>,
) -> Option<&'a Vec<ResolvedField>> {
    if !cache.contains_key(name) {
        let raw = formats.get(name)?;
        let mut out = Vec::with_capacity(raw.len());
        for (type_token, fname) in raw {
            let (base_str, array_len) = split_array(type_token);
            let (base, elem) = match Base::from_str(base_str) {
                Some(b) => (Some(b), b.size()),
                None => {
                    // Nested struct: size = struct size * array_len (skipped).
                    let sz = struct_size(base_str, formats, 0)?;
                    (None, sz)
                }
            };
            out.push(ResolvedField {
                name: fname.clone(),
                base,
                array_len,
                total_size: elem * array_len,
            });
        }
        cache.insert(name.to_string(), out);
    }
    cache.get(name)
}

/// Recursively compute a (possibly nested) struct's serialized size in bytes.
fn struct_size(name: &str, formats: &BTreeMap<String, Vec<RawField>>, depth: u8) -> Option<usize> {
    if depth > 16 {
        return None;
    }
    let raw = formats.get(name)?;
    let mut total = 0usize;
    for (type_token, _) in raw {
        let (base_str, array_len) = split_array(type_token);
        let elem = match Base::from_str(base_str) {
            Some(b) => b.size(),
            None => struct_size(base_str, formats, depth + 1)?,
        };
        total += elem * array_len;
    }
    Some(total)
}

/// Split a type token like `float[3]` into `("float", 3)`.
fn split_array(token: &str) -> (&str, usize) {
    if let Some(open) = token.find('[') {
        let base = &token[..open];
        let len = token[open + 1..]
            .trim_end_matches(']')
            .parse::<usize>()
            .unwrap_or(1);
        (base, len.max(1))
    } else {
        (token, 1)
    }
}

/// Parse an 'F' format payload: `name:type field;type field;...`.
fn parse_format(payload: &[u8]) -> Option<(String, Vec<RawField>)> {
    let s = String::from_utf8_lossy(payload);
    let (name, body) = s.split_once(':')?;
    let mut fields = Vec::new();
    for part in body.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((ty, fname)) = part.split_once(' ') {
            fields.push((ty.trim().to_string(), fname.trim().to_string()));
        }
    }
    Some((name.to_string(), fields))
}

/// Parse an 'I' info payload: `keylen(u8) | "type key" | value`.
fn parse_info(payload: &[u8]) -> Option<(String, String)> {
    if payload.is_empty() {
        return None;
    }
    let key_len = payload[0] as usize;
    if 1 + key_len > payload.len() {
        return None;
    }
    let key_field = String::from_utf8_lossy(&payload[1..1 + key_len]).into_owned();
    let value_bytes = &payload[1 + key_len..];
    let (ty, key) = key_field
        .split_once(' ')
        .unwrap_or(("", key_field.as_str()));
    // Render value: strings as text, otherwise best-effort numeric.
    let value = if ty.starts_with("char") {
        String::from_utf8_lossy(value_bytes)
            .trim_end_matches('\0')
            .to_string()
    } else if ty == "int32_t" && value_bytes.len() >= 4 {
        i32::from_le_bytes(value_bytes[0..4].try_into().unwrap()).to_string()
    } else if ty == "uint32_t" && value_bytes.len() >= 4 {
        u32::from_le_bytes(value_bytes[0..4].try_into().unwrap()).to_string()
    } else if ty == "float" && value_bytes.len() >= 4 {
        f32::from_le_bytes(value_bytes[0..4].try_into().unwrap()).to_string()
    } else {
        format!("{} bytes", value_bytes.len())
    };
    Some((key.to_string(), value))
}

/// Build flight-mode spans from `vehicle_status[*].nav_state` if present.
fn build_modes(series: &BTreeMap<String, Series>) -> Vec<ModeSpan> {
    let key = series
        .keys()
        .find(|k| k.starts_with("vehicle_status[") && k.ends_with("].nav_state"))
        .cloned();
    let Some(key) = key else {
        return Vec::new();
    };
    let s = &series[&key];
    let mut spans: Vec<ModeSpan> = Vec::new();
    for (i, &t) in s.t.iter().enumerate() {
        let mode = nav_state_name(s.v[i] as i64);
        match spans.last_mut() {
            Some(last) if last.mode == mode => last.t1 = t,
            _ => spans.push(ModeSpan { mode, t0: t, t1: t }),
        }
    }
    spans
}

/// PX4 `nav_state` → human name (common subset).
pub fn nav_state_name(n: i64) -> String {
    let s = match n {
        0 => "MANUAL",
        1 => "ALTCTL",
        2 => "POSCTL",
        3 => "AUTO_MISSION",
        4 => "AUTO_LOITER",
        5 => "AUTO_RTL",
        10 => "ACRO",
        12 => "DESCEND",
        13 => "TERMINATION",
        14 => "OFFBOARD",
        15 => "STAB",
        17 => "AUTO_TAKEOFF",
        18 => "AUTO_LAND",
        19 => "AUTO_FOLLOW",
        20 => "AUTO_PRECLAND",
        _ => return format!("MODE_{n}"),
    };
    s.to_string()
}

fn track_min(min_ts: &mut Option<u64>, ts: u64) {
    if ts == 0 {
        return;
    }
    *min_ts = Some(min_ts.map_or(ts, |m| m.min(ts)));
}

/// Little byte cursor over the ULog buffer.
struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Cursor { buf, pos: 0 }
    }

    /// Read the next `(msg_type, payload)`; `None` at end / on truncation.
    fn next_message(&mut self) -> Option<(u8, &'a [u8])> {
        if self.pos + 3 > self.buf.len() {
            return None;
        }
        let size = u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos + 1]]) as usize;
        let mtype = self.buf[self.pos + 2];
        let start = self.pos + 3;
        let end = start + size;
        if end > self.buf.len() {
            return None;
        }
        self.pos = end;
        Some((mtype, &self.buf[start..end]))
    }
}

/// Convenience for callers that have a path.
pub fn parse_file(path: &str) -> Result<Log> {
    let bytes = std::fs::read(path).map_err(|e| anyhow!("read {path}: {e}"))?;
    let name = std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    parse(&bytes, path, &name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_array_parses_dimensions() {
        assert_eq!(split_array("float"), ("float", 1));
        assert_eq!(split_array("float[3]"), ("float", 3));
        assert_eq!(split_array("uint8_t[16]"), ("uint8_t", 16));
    }

    #[test]
    fn rejects_non_ulog() {
        assert!(parse(b"not a ulog file at all", "x", "x").is_err());
    }

    #[test]
    fn nav_state_names() {
        assert_eq!(nav_state_name(2), "POSCTL");
        assert_eq!(nav_state_name(5), "AUTO_RTL");
        assert_eq!(nav_state_name(99), "MODE_99");
    }
}
