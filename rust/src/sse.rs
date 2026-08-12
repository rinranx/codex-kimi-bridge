use crate::error::{BridgeError, BridgeResult};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    pub event: Option<String>,
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseDecoder {
    buffer: Vec<u8>,
    pending_cr: bool,
}

impl SseDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> BridgeResult<Vec<SseFrame>> {
        for byte in chunk {
            if self.pending_cr {
                self.buffer.push(b'\n');
                self.pending_cr = false;
                if *byte == b'\n' {
                    continue;
                }
            }
            if *byte == b'\r' {
                self.pending_cr = true;
            } else {
                self.buffer.push(*byte);
            }
        }
        self.drain_frames()
    }

    pub fn finish(&mut self) -> BridgeResult<Vec<SseFrame>> {
        if self.pending_cr {
            self.buffer.push(b'\n');
            self.pending_cr = false;
        }
        let mut frames = self.drain_frames()?;
        if !self.buffer.is_empty() {
            let trailing = std::mem::take(&mut self.buffer);
            if let Some(frame) = parse_block(&trailing)? {
                frames.push(frame);
            }
        }
        Ok(frames)
    }

    fn drain_frames(&mut self) -> BridgeResult<Vec<SseFrame>> {
        let mut frames = Vec::new();
        while let Some(boundary) = self.buffer.windows(2).position(|window| window == b"\n\n") {
            let block = self.buffer.drain(..boundary).collect::<Vec<_>>();
            self.buffer.drain(..2);
            if let Some(frame) = parse_block(&block)? {
                frames.push(frame);
            }
        }
        Ok(frames)
    }
}

fn parse_block(block: &[u8]) -> BridgeResult<Option<SseFrame>> {
    if block.is_empty() {
        return Ok(None);
    }
    let text = std::str::from_utf8(block).map_err(|_| {
        BridgeError::new("The upstream SSE stream was not valid UTF-8.")
            .status(502)
            .kind("upstream_protocol_error")
            .code("invalid_upstream_sse")
    })?;
    let mut event = None;
    let mut data = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            event = Some(value.trim().to_owned());
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.strip_prefix(' ').unwrap_or(value).to_owned());
        }
    }
    if data.is_empty() {
        return Ok(None);
    }
    Ok(Some(SseFrame {
        event,
        data: data.join("\n"),
    }))
}

pub fn encode_event(event: &Value) -> Vec<u8> {
    let kind = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("message");
    format!("event: {kind}\ndata: {}\n\n", event).into_bytes()
}

pub fn encode_done() -> &'static [u8] {
    b"data: [DONE]\n\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chunked_crlf_and_multiline_data() {
        let mut decoder = SseDecoder::default();
        assert!(decoder.push(b"event: note\r").unwrap().is_empty());
        let frames = decoder.push(b"\ndata: one\r\ndata: two\r\n\r\n").unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event.as_deref(), Some("note"));
        assert_eq!(frames[0].data, "one\ntwo");
    }
}
