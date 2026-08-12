use serde_json::{Value, json};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct BridgeError {
    pub message: String,
    pub status: u16,
    pub kind: String,
    pub code: String,
    pub param: Option<String>,
}

impl BridgeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: 400,
            kind: "invalid_request_error".into(),
            code: "invalid_request_error".into(),
            param: None,
        }
    }

    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    pub fn kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = kind.into();
        self
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.code = code.into();
        self
    }

    pub fn param(mut self, param: impl Into<String>) -> Self {
        self.param = Some(param.into());
        self
    }

    pub fn envelope(&self) -> Value {
        json!({
            "error": {
                "message": self.message,
                "type": self.kind,
                "code": self.code,
                "param": self.param,
            }
        })
    }

    pub fn stream_event(&self, sequence_number: u64) -> Value {
        json!({
            "type": "error",
            "sequence_number": sequence_number,
            "message": self.message,
            "code": self.code,
            "param": self.param,
        })
    }
}

impl Display for BridgeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BridgeError {}

pub type BridgeResult<T> = Result<T, BridgeError>;

pub fn sanitize_provider_error(value: &Value, fallback_status: u16) -> Value {
    let source = value.get("error").unwrap_or(value);
    let message = source
        .get("message")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("The upstream provider returned HTTP {fallback_status}."));
    let kind = source
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("upstream_provider_error");
    let code = source
        .get("code")
        .cloned()
        .unwrap_or_else(|| Value::String(format!("http_{fallback_status}")));
    let param = match source.get("param") {
        Some(Value::String(value)) => Value::String(value.clone()),
        Some(Value::Null) | None => Value::Null,
        _ => Value::Null,
    };
    json!({
        "error": {
            "message": message,
            "type": kind,
            "code": code,
            "param": param,
        }
    })
}
