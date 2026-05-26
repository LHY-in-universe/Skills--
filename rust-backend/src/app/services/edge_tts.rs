//! Edge TTS 流式合成。
//!
//! 忠实移植 Python `edge_tts` 包的核心 WSS 协议，只保留流式音频输出，
//! 不实现 WordBoundary/SentenceBoundary 元数据解析。

use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_tungstenite::tungstenite;

const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const CHROMIUM_FULL_VERSION: &str = "143.0.3650.75";
const BASE_URL: &str = "speech.platform.bing.com/consumer/speech/synthesize/readaloud";
const WIN_EPOCH: i64 = 11644473600;
const MAX_CHUNK_BYTES: usize = 4096;

static CLOCK_SKEW_SECS: AtomicI64 = AtomicI64::new(0);

fn wss_url(connection_id: &str, sec_ms_gec: &str) -> String {
    format!(
        "wss://{BASE_URL}/edge/v1?TrustedClientToken={TRUSTED_CLIENT_TOKEN}\
         &ConnectionId={connection_id}\
         &Sec-MS-GEC={sec_ms_gec}\
         &Sec-MS-GEC-Version=1-{CHROMIUM_FULL_VERSION}"
    )
}

fn chromium_major() -> &'static str {
    CHROMIUM_FULL_VERSION.split('.').next().unwrap_or("143")
}

fn wss_headers() -> Vec<(String, String)> {
    let major = chromium_major();
    vec![
        ("Pragma".into(), "no-cache".into()),
        ("Cache-Control".into(), "no-cache".into()),
        (
            "Origin".into(),
            "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold".into(),
        ),
        ("Sec-WebSocket-Version".into(), "13".into()),
        (
            "User-Agent".into(),
            format!(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/{major}.0.0.0 Safari/537.36 Edg/{major}.0.0.0"
            ),
        ),
        ("Accept-Encoding".into(), "gzip, deflate, br, zstd".into()),
        ("Accept-Language".into(), "en-US,en;q=0.9".into()),
    ]
}

fn generate_sec_ms_gec() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as f64)
        .unwrap_or(0.0);
    let skew = CLOCK_SKEW_SECS.load(Ordering::Relaxed) as f64;
    let mut ticks = now + skew;
    ticks += WIN_EPOCH as f64;
    ticks -= ticks % 300.0;
    ticks *= 1e7;
    let str_to_hash = format!("{:.0}{TRUSTED_CLIENT_TOKEN}", ticks);
    let hash = Sha256::digest(str_to_hash.as_bytes());
    format!("{:X}", hash)
}

fn generate_muid() -> String {
    let bytes: [u8; 16] = rand::random();
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn connect_id() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn remove_incompatible_chars(s: &str) -> String {
    s.chars()
        .map(|c| {
            let code = c as u32;
            if (0..=8).contains(&code) || (11..=12).contains(&code) || (14..=31).contains(&code) {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn mk_ssml(voice: &str, escaped_text: &str) -> String {
    format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>\
         <voice name='{voice}'>\
         <prosody pitch='+0Hz' rate='+0%' volume='+0%'>\
         {escaped_text}\
         </prosody></voice></speak>"
    )
}

fn date_to_string() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let day_of_week = ((days + 4) % 7) as usize; // 1970-01-01 is Thursday (4)
    let weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md as i64 {
            m = i;
            break;
        }
        remaining -= md as i64;
    }
    let day = remaining + 1;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let mi = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    format!(
        "{} {} {:02} {} {:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)",
        weekdays[day_of_week], months[m], day, y, h, mi, s
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn ssml_headers_plus_data(request_id: &str, timestamp: &str, ssml: &str) -> String {
    format!(
        "X-RequestId:{request_id}\r\n\
         Content-Type:application/ssml+xml\r\n\
         X-Timestamp:{timestamp}Z\r\n\
         Path:ssml\r\n\r\n\
         {ssml}"
    )
}

fn speech_config_message() -> String {
    format!(
        "X-Timestamp:{}\r\n\
         Content-Type:application/json; charset=utf-8\r\n\
         Path:speech.config\r\n\r\n\
         {{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\
         \"sentenceBoundaryEnabled\":\"true\",\"wordBoundaryEnabled\":\"false\"\
         }},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}",
        date_to_string()
    )
}

fn split_text_by_byte_length(text: &str, byte_length: usize) -> Vec<Vec<u8>> {
    let mut bytes = text.as_bytes().to_vec();
    let mut chunks = Vec::new();

    while bytes.len() > byte_length {
        let mut split_at = -1i64;

        // Find last newline or space within limit
        for i in (0..byte_length).rev() {
            if bytes[i] == b'\n' || bytes[i] == b' ' {
                split_at = i as i64;
                break;
            }
        }

        if split_at < 0 {
            // Find safe UTF-8 split point
            let mut pos = byte_length;
            while pos > 0 {
                if std::str::from_utf8(&bytes[..pos]).is_ok() {
                    split_at = pos as i64;
                    break;
                }
                pos -= 1;
            }
        }

        // Adjust for XML entities
        if split_at > 0 {
            let sa = split_at as usize;
            if let Some(amp_pos) = bytes[..sa].iter().rposition(|&b| b == b'&') {
                let has_semi = bytes[amp_pos..sa].contains(&b';');
                if !has_semi {
                    split_at = amp_pos as i64;
                }
            }
        }

        if split_at <= 0 {
            split_at = 1;
        }

        let sa = split_at as usize;
        let chunk: Vec<u8> = bytes[..sa].to_vec();
        let trimmed = trim_bytes(&chunk);
        if !trimmed.is_empty() {
            chunks.push(trimmed.to_vec());
        }
        bytes = bytes[sa..].to_vec();
    }

    let trimmed = trim_bytes(&bytes);
    if !trimmed.is_empty() {
        chunks.push(trimmed.to_vec());
    }
    chunks
}

fn trim_bytes(b: &[u8]) -> &[u8] {
    let start = b
        .iter()
        .position(|&c| !c.is_ascii_whitespace())
        .unwrap_or(b.len());
    let end = b
        .iter()
        .rposition(|&c| !c.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(start);
    &b[start..end]
}

fn parse_binary_audio(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 2 {
        return None;
    }
    let header_length = u16::from_be_bytes([data[0], data[1]]) as usize;
    if header_length + 2 > data.len() {
        return None;
    }
    let header_section = &data[2..2 + header_length];
    if let Some(ct_line) = header_section
        .windows(13)
        .position(|w| w == b"Content-Type:")
    {
        let rest = &header_section[ct_line..];
        if let Some(end) = rest.iter().position(|&b| b == b'\r') {
            let ct_value = &rest[13..end];
            let ct_str = std::str::from_utf8(ct_value).unwrap_or("").trim();
            if ct_str != "audio/mpeg" {
                return None;
            }
        }
    }
    let audio_start = 2 + header_length;
    if audio_start >= data.len() {
        return None;
    }
    Some(data[audio_start..].to_vec())
}

fn parse_text_path(data: &str) -> Option<&str> {
    for line in data.split("\r\n") {
        if let Some(rest) = line.strip_prefix("Path:") {
            return Some(rest.trim());
        }
    }
    None
}

/// 对一段文本执行流式 Edge TTS 合成，通过 callback 逐块返回 MP3 字节。
pub async fn stream_audio<F>(text: &str, voice: &str, mut on_audio: F) -> anyhow::Result<()>
where
    F: FnMut(Vec<u8>) + Send,
{
    let cleaned = remove_incompatible_chars(text);
    let escaped = xml_escape(&cleaned);
    let chunks = split_text_by_byte_length(&escaped, MAX_CHUNK_BYTES);

    for chunk_bytes in chunks {
        let chunk_text = String::from_utf8_lossy(&chunk_bytes);
        if let Err(e) = stream_single_chunk(&chunk_text, voice, &mut on_audio).await {
            // 403 → adjust clock skew and retry once
            let msg = e.to_string();
            if msg.contains("403") || msg.contains("Forbidden") {
                tracing::warn!("Edge TTS 403, adjusting clock skew and retrying");
                CLOCK_SKEW_SECS.fetch_add(300, Ordering::Relaxed);
                stream_single_chunk(&chunk_text, voice, &mut on_audio).await?;
            } else {
                return Err(e);
            }
        }
    }
    Ok(())
}

async fn stream_single_chunk<F>(
    escaped_text: &str,
    voice: &str,
    on_audio: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(Vec<u8>) + Send,
{
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let conn_id = connect_id();
    let sec_ms_gec = generate_sec_ms_gec();
    let url = wss_url(&conn_id, &sec_ms_gec);
    let muid = generate_muid();

    let mut request = url.into_client_request()?;
    for (k, v) in wss_headers() {
        request.headers_mut().insert(
            http::header::HeaderName::from_bytes(k.as_bytes())?,
            http::header::HeaderValue::from_str(&v)?,
        );
    }
    request.headers_mut().insert(
        http::header::COOKIE,
        http::header::HeaderValue::from_str(&format!("muid={muid};"))?,
    );

    let (ws_stream, _) = tokio_tungstenite::connect_async(request).await?;
    let (mut write, mut read) = ws_stream.split();

    // 1. Send speech.config
    write
        .send(tungstenite::Message::Text(speech_config_message().into()))
        .await?;

    // 2. Send SSML
    let ssml = mk_ssml(voice, escaped_text);
    let ssml_msg = ssml_headers_plus_data(&connect_id(), &date_to_string(), &ssml);
    write
        .send(tungstenite::Message::Text(ssml_msg.into()))
        .await?;

    // 3. Receive audio
    while let Some(msg) = read.next().await {
        let msg = msg?;
        match msg {
            tungstenite::Message::Binary(data) => {
                if let Some(audio) = parse_binary_audio(&data) {
                    if !audio.is_empty() {
                        on_audio(audio);
                    }
                }
            }
            tungstenite::Message::Text(text) => {
                if let Some(path) = parse_text_path(&text) {
                    if path == "turn.end" {
                        break;
                    }
                }
            }
            tungstenite::Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_escape() {
        assert_eq!(
            xml_escape("a&b<c>d\"e'f"),
            "a&amp;b&lt;c&gt;d&quot;e&apos;f"
        );
        assert_eq!(xml_escape("hello"), "hello");
    }

    #[test]
    fn test_mk_ssml() {
        let ssml = mk_ssml("zh-CN-XiaoxiaoNeural", "你好");
        assert!(ssml.contains("name='zh-CN-XiaoxiaoNeural'"));
        assert!(ssml.contains("你好"));
        assert!(ssml.starts_with("<speak"));
        assert!(ssml.ends_with("</speak>"));
    }

    #[test]
    fn test_remove_incompatible_chars() {
        let s = "hello\x0bworld\x01test";
        let cleaned = remove_incompatible_chars(s);
        assert_eq!(cleaned, "hello world test");
    }

    #[test]
    fn test_split_text_by_byte_length() {
        let short = "hello world";
        let chunks = split_text_by_byte_length(short, 4096);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], b"hello world");

        let long = "a ".repeat(3000);
        let chunks = split_text_by_byte_length(&long, 4096);
        assert!(chunks.len() >= 1);
        for c in &chunks {
            assert!(c.len() <= 4096);
        }
    }

    #[test]
    fn test_parse_binary_audio() {
        // header_length = 10, then 10 bytes of header, then audio data
        let mut data = vec![0u8, 10];
        data.extend_from_slice(b"Content-Ty"); // 10 bytes header
        data.extend_from_slice(b"\xAA\xBB\xCC"); // audio
                                                 // No Content-Type: audio/mpeg header present, but audio data after header
        let result = parse_binary_audio(&data);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_generate_sec_ms_gec_format() {
        let token = generate_sec_ms_gec();
        assert_eq!(token.len(), 64); // SHA256 hex = 64 chars
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(token, token.to_uppercase());
    }

    #[test]
    fn test_date_to_string_format() {
        let s = date_to_string();
        assert!(s.contains("GMT+0000 (Coordinated Universal Time)"));
    }
}
