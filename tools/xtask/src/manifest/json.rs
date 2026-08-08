//! Hand-rolled JSON emission for the release manifest.
//!
//! The workspace carries no JSON serialization dependency and the
//! conformance golden is written the same way, by formatting a fixed key
//! order into lines. Determinism is the point: the guard regenerates the
//! manifest and compares it byte for byte, so the document may not
//! depend on map iteration order, on a locale, or on how a formatter
//! chooses to abbreviate a float.

use crate::error::XtaskError;

/// A JSON string literal, with the escapes JSON requires.
pub fn string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A `f32` as a JSON number.
///
/// Fixed precision rather than the shortest round-trip form, so the
/// bytes depend only on the value and not on the formatter's choice of
/// abbreviation. Six decimals resolve more than an `f32` ulp at every
/// magnitude a design frame reaches, so two different frames cannot
/// print the same digits; trailing zeros are then trimmed to keep whole
/// pixel counts readable as `480` rather than `480.000000`.
pub fn number(value: f32, field: &'static str) -> Result<String, XtaskError> {
    if !value.is_finite() {
        return Err(XtaskError::UnpinnableValue {
            value: field,
            reason: format!("{value} is not a finite number, so no JSON number states it"),
        });
    }
    let mut text = format!("{value:.6}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    Ok(text)
}

/// Lowercase hex of a digest. Bare, so a caller can compare it against a
/// pinned literal before wrapping it in [`string`].
pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// `"width": w, "height": h` for a design frame, inline.
pub fn frame_fields(width: f32, height: f32) -> Result<String, XtaskError> {
    Ok(format!(
        "\"width\": {}, \"height\": {}",
        number(width, "frame width")?,
        number(height, "frame height")?
    ))
}
