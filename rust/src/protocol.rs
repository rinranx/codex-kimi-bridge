use crate::error::{BridgeError, BridgeResult};
use crate::reasoning::ReasoningStore;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Function,
    Custom,
}

impl ToolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Custom => "custom",
        }
    }
}

#[derive(Clone)]
pub struct TranslationContext {
    pub model: String,
    pub original_request: Value,
    pub tool_map: BTreeMap<String, ToolKind>,
    pub prompt_cache_key: String,
    pub reasoning_store: Option<Arc<ReasoningStore>>,
}

pub struct TranslatedRequest {
    pub body: Value,
    pub context: TranslationContext,
}

pub fn translate_responses_request(
    input: Value,
    default_model: &str,
    reasoning_store: Option<Arc<ReasoningStore>>,
) -> BridgeResult<TranslatedRequest> {
    let request = input
        .as_object()
        .ok_or_else(|| BridgeError::new("request body must be a JSON object."))?;

    if request
        .get("previous_response_id")
        .is_some_and(|value| !value.is_null() && value.as_str().is_none_or(|text| !text.is_empty()))
    {
        return Err(BridgeError::new(
            "previous_response_id is not supported because Kimi Chat Completions is stateless. Send the conversation items in input instead.",
        )
        .param("previous_response_id")
        .code("unsupported_parameter"));
    }

    let model = request
        .get("model")
        .and_then(non_empty_string)
        .unwrap_or(default_model)
        .to_owned();
    let mut messages = Vec::new();

    if let Some(instructions) = request.get("instructions").filter(|value| !value.is_null()) {
        messages.push(json!({
            "role": "system",
            "content": coerce_instruction_text(instructions)?,
        }));
    }

    append_responses_input(
        &mut messages,
        request.get("input"),
        reasoning_store.as_deref(),
    )?;
    if messages.is_empty() {
        return Err(BridgeError::new("input must contain at least one message.")
            .param("input")
            .code("missing_required_parameter"));
    }

    let empty_tools = Value::Array(Vec::new());
    let (chat_tools, tool_map) = translate_tools(request.get("tools").unwrap_or(&empty_tools))?;
    let stream = request.get("stream").and_then(Value::as_bool) != Some(false);
    let prompt_cache_key = derive_prompt_cache_key(&input, &model, &messages);
    let mut body = Map::new();
    body.insert("model".into(), Value::String(model.clone()));
    body.insert("messages".into(), Value::Array(messages));
    body.insert("stream".into(), Value::Bool(stream));
    body.insert(
        "prompt_cache_key".into(),
        Value::String(prompt_cache_key.clone()),
    );
    if stream {
        body.insert("stream_options".into(), json!({ "include_usage": true }));
    }
    if !chat_tools.is_empty() {
        body.insert("tools".into(), Value::Array(chat_tools));
    }
    if let Some(choice) = translate_tool_choice(request.get("tool_choice"), &tool_map)? {
        body.insert("tool_choice".into(), choice);
    }
    if let Some(effort) = translate_reasoning_effort(
        request
            .get("reasoning")
            .and_then(Value::as_object)
            .and_then(|value| value.get("effort")),
    )? {
        body.insert("reasoning_effort".into(), Value::String(effort.into()));
    }
    if let Some(tokens) = request.get("max_output_tokens").and_then(positive_integer) {
        body.insert("max_completion_tokens".into(), Value::Number(tokens.into()));
    }
    if let Some(format) = translate_response_format(
        request
            .get("text")
            .and_then(Value::as_object)
            .and_then(|value| value.get("format")),
    )? {
        body.insert("response_format".into(), format);
    }
    for field in ["temperature", "top_p", "seed", "stop"] {
        if let Some(value) = request.get(field).filter(|value| !value.is_null()) {
            body.insert(field.into(), value.clone());
        }
    }
    if let Some(identifier) = request.get("safety_identifier").and_then(non_empty_string) {
        body.insert(
            "safety_identifier".into(),
            Value::String(identifier.to_owned()),
        );
    }

    Ok(TranslatedRequest {
        body: Value::Object(body),
        context: TranslationContext {
            model,
            original_request: input,
            tool_map,
            prompt_cache_key,
            reasoning_store,
        },
    })
}

pub fn translate_chat_completion(
    chat: &Value,
    context: &TranslationContext,
) -> BridgeResult<Value> {
    let object = chat.as_object().ok_or_else(|| {
        BridgeError::new("upstream response must be a JSON object.")
            .status(502)
            .kind("upstream_protocol_error")
            .code("invalid_upstream_response")
    })?;
    let choice = object
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(Value::as_object)
        .ok_or_else(|| {
            BridgeError::new("The upstream response did not contain choices[0].")
                .status(502)
                .kind("upstream_protocol_error")
                .code("invalid_upstream_response")
        })?;
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(reasoning) = message.get("reasoning_content").and_then(non_empty_string) {
        remember_reasoning_for_tool_calls(
            context.reasoning_store.as_deref(),
            &tool_calls,
            reasoning,
        );
    }

    let mut output = Vec::new();
    let text = normalize_assistant_text(message.get("content"));
    if !text.is_empty() {
        output.push(make_completed_message(&text, None));
    }
    for tool_call in &tool_calls {
        output.push(make_completed_tool_call(tool_call, &context.tool_map));
    }

    let incomplete = choice.get("finish_reason").and_then(Value::as_str) == Some("length");
    Ok(make_response_object(
        response_id_from(object.get("id").and_then(Value::as_str)),
        object.get("created").and_then(Value::as_i64),
        object
            .get("model")
            .and_then(non_empty_string)
            .unwrap_or(&context.model),
        if incomplete {
            "incomplete"
        } else {
            "completed"
        },
        output,
        Some(normalize_usage(object.get("usage"))),
        incomplete.then_some("max_output_tokens"),
        &context.original_request,
    ))
}

fn append_responses_input(
    messages: &mut Vec<Value>,
    input: Option<&Value>,
    reasoning_store: Option<&ReasoningStore>,
) -> BridgeResult<()> {
    match input {
        Some(Value::String(text)) => {
            messages.push(json!({ "role": "user", "content": text }));
            return Ok(());
        }
        None | Some(Value::Null) => return Ok(()),
        Some(Value::Array(items)) => {
            for item in items {
                let object = item.as_object().ok_or_else(|| {
                    BridgeError::new("Every input item must be an object.").param("input")
                })?;
                let item_type = object.get("type").and_then(Value::as_str);
                if matches!(item_type, Some("reasoning" | "item_reference")) {
                    continue;
                }
                if item_type == Some("message")
                    || object.get("role").and_then(non_empty_string).is_some()
                {
                    messages.push(translate_message(object)?);
                } else if item_type == Some("function_call") {
                    let call_id = object
                        .get("call_id")
                        .or_else(|| object.get("id"))
                        .and_then(non_empty_string)
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| make_id("call"));
                    let tool_call = json!({
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": object.get("name").cloned().unwrap_or(Value::Null),
                            "arguments": stringify_arguments(object.get("arguments")),
                        }
                    });
                    append_assistant_tool_call(
                        messages,
                        tool_call,
                        reasoning_store.and_then(|store| store.get(&call_id)),
                    );
                } else if item_type == Some("custom_tool_call") {
                    let call_id = object
                        .get("call_id")
                        .or_else(|| object.get("id"))
                        .and_then(non_empty_string)
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| make_id("call"));
                    let input = coerce_tool_output(object.get("input"));
                    let tool_call = json!({
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": object.get("name").cloned().unwrap_or(Value::Null),
                            "arguments": serde_json::to_string(&json!({ "input": input })).unwrap_or_else(|_| "{}".into()),
                        }
                    });
                    append_assistant_tool_call(
                        messages,
                        tool_call,
                        reasoning_store.and_then(|store| store.get(&call_id)),
                    );
                } else if matches!(
                    item_type,
                    Some("function_call_output" | "custom_tool_call_output")
                ) {
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": object.get("call_id").cloned().unwrap_or(Value::Null),
                        "content": coerce_tool_output(object.get("output")),
                    }));
                } else {
                    return Err(BridgeError::new(format!(
                        "Unsupported Responses input item type: {}.",
                        item_type.unwrap_or("unknown")
                    ))
                    .param("input")
                    .code("unsupported_input_item"));
                }
            }
            Ok(())
        }
        Some(_) => Err(
            BridgeError::new("input must be a string or an array of Responses items.")
                .param("input"),
        ),
    }
}

fn translate_message(item: &Map<String, Value>) -> BridgeResult<Value> {
    let raw_role = item
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let role = if raw_role == "developer" {
        "system"
    } else {
        raw_role
    };
    if !matches!(role, "system" | "user" | "assistant" | "tool") {
        return Err(
            BridgeError::new(format!("Unsupported message role: {raw_role}."))
                .param("input")
                .code("unsupported_message_role"),
        );
    }
    let mut message = Map::new();
    message.insert("role".into(), Value::String(role.into()));
    message.insert(
        "content".into(),
        translate_content(item.get("content"), role)?,
    );
    if let Some(name) = item.get("name").and_then(non_empty_string) {
        message.insert("name".into(), Value::String(name.into()));
    }
    if role == "tool" {
        if let Some(call_id) = item
            .get("tool_call_id")
            .or_else(|| item.get("call_id"))
            .and_then(non_empty_string)
        {
            message.insert("tool_call_id".into(), Value::String(call_id.into()));
        }
    }
    Ok(Value::Object(message))
}

fn translate_content(content: Option<&Value>, role: &str) -> BridgeResult<Value> {
    match content {
        Some(Value::String(text)) => Ok(Value::String(text.clone())),
        Some(Value::Array(parts)) => {
            let mut translated = Vec::new();
            for part in parts {
                let Some(object) = part.as_object() else {
                    continue;
                };
                let part_type = object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                match part_type {
                    "input_text" | "output_text" | "text" => translated.push(json!({
                        "type": "text",
                        "text": object.get("text").and_then(Value::as_str).unwrap_or("")
                    })),
                    "refusal" => translated.push(json!({
                        "type": "text",
                        "text": object.get("refusal").and_then(Value::as_str).unwrap_or("")
                    })),
                    "input_image" | "image_url" => {
                        if role != "user" {
                            return Err(BridgeError::new(
                                "Image content is only supported in user messages.",
                            )
                            .param("input")
                            .code("unsupported_content_part"));
                        }
                        let raw = object.get("image_url").or_else(|| object.get("url"));
                        let url = raw
                            .and_then(|value| {
                                value
                                    .as_str()
                                    .or_else(|| value.get("url").and_then(Value::as_str))
                            })
                            .and_then(non_empty_text)
                            .ok_or_else(|| {
                                BridgeError::new("An input_image part must contain image_url.")
                                    .param("input")
                            })?;
                        let detail = object.get("detail").and_then(Value::as_str).or_else(|| {
                            raw.and_then(|value| value.get("detail"))
                                .and_then(Value::as_str)
                        });
                        let mut image = Map::new();
                        image.insert("url".into(), Value::String(url.into()));
                        if let Some(detail) = detail {
                            image.insert("detail".into(), Value::String(detail.into()));
                        }
                        translated.push(json!({ "type": "image_url", "image_url": image }));
                    }
                    "input_video" | "video_url" => {
                        if role != "user" {
                            return Err(BridgeError::new(
                                "Video content is only supported in user messages.",
                            )
                            .param("input")
                            .code("unsupported_content_part"));
                        }
                        let raw = object.get("video_url").or_else(|| object.get("url"));
                        let url = raw
                            .and_then(|value| {
                                value
                                    .as_str()
                                    .or_else(|| value.get("url").and_then(Value::as_str))
                            })
                            .and_then(non_empty_text)
                            .ok_or_else(|| {
                                BridgeError::new("An input_video part must contain video_url.")
                                    .param("input")
                            })?;
                        translated.push(json!({
                            "type": "video_url",
                            "video_url": { "url": url }
                        }));
                    }
                    _ if object.get("text").and_then(Value::as_str).is_some() => {
                        translated.push(json!({
                            "type": "text",
                            "text": object.get("text").and_then(Value::as_str).unwrap_or("")
                        }));
                    }
                    _ => {
                        return Err(BridgeError::new(format!(
                            "Unsupported content part type: {part_type}."
                        ))
                        .param("input")
                        .code("unsupported_content_part"));
                    }
                }
            }
            if translated
                .iter()
                .all(|part| part.get("type").and_then(Value::as_str) == Some("text"))
            {
                Ok(Value::String(
                    translated
                        .iter()
                        .filter_map(|part| part.get("text").and_then(Value::as_str))
                        .collect::<String>(),
                ))
            } else {
                Ok(Value::Array(translated))
            }
        }
        Some(value) => Ok(Value::String(coerce_tool_output(Some(value)))),
        None => Ok(Value::String(String::new())),
    }
}

fn translate_tools(tools: &Value) -> BridgeResult<(Vec<Value>, BTreeMap<String, ToolKind>)> {
    let tools = tools
        .as_array()
        .ok_or_else(|| BridgeError::new("tools must be an array.").param("tools"))?;
    let mut chat_tools = Vec::new();
    let mut tool_map = BTreeMap::new();
    for tool in tools {
        let object = tool
            .as_object()
            .ok_or_else(|| BridgeError::new("Every tool must be an object.").param("tools"))?;
        match object.get("type").and_then(Value::as_str) {
            Some("function") => {
                let definition = object
                    .get("function")
                    .and_then(Value::as_object)
                    .unwrap_or(object);
                let name = object
                    .get("name")
                    .or_else(|| definition.get("name"))
                    .and_then(non_empty_string)
                    .ok_or_else(|| {
                        BridgeError::new("Every function or custom tool must have a name.")
                            .param("tools")
                    })?;
                let mut function = Map::new();
                function.insert("name".into(), Value::String(name.into()));
                function.insert(
                    "description".into(),
                    definition
                        .get("description")
                        .cloned()
                        .unwrap_or_else(|| Value::String(String::new())),
                );
                function.insert(
                    "parameters".into(),
                    definition.get("parameters").cloned().unwrap_or_else(|| {
                        json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false,
                        })
                    }),
                );
                if let Some(strict) = definition.get("strict").and_then(Value::as_bool) {
                    function.insert("strict".into(), Value::Bool(strict));
                }
                chat_tools.push(json!({ "type": "function", "function": function }));
                tool_map.insert(name.into(), ToolKind::Function);
            }
            Some("custom") => {
                let name = object
                    .get("name")
                    .and_then(non_empty_string)
                    .ok_or_else(|| {
                        BridgeError::new("Every function or custom tool must have a name.")
                            .param("tools")
                    })?;
                let description = object
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("Accepts free-form text input.");
                let format_note = object
                    .get("format")
                    .map(|format| format!("\nOriginal input constraint: {format}"))
                    .unwrap_or_default();
                chat_tools.push(json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": format!(
                            "{description}\nReturn the exact free-form tool input in the JSON field \"input\".{format_note}"
                        ),
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "input": {
                                    "type": "string",
                                    "description": "Exact free-form input for the tool."
                                }
                            },
                            "required": ["input"],
                            "additionalProperties": false,
                        },
                        "strict": true,
                    }
                }));
                tool_map.insert(name.into(), ToolKind::Custom);
            }
            other => {
                return Err(BridgeError::new(format!(
                    "Unsupported Responses tool type: {}. Only function and custom tools can be translated safely.",
                    other.unwrap_or("unknown")
                ))
                .param("tools")
                .code("unsupported_tool_type"));
            }
        }
    }
    Ok((chat_tools, tool_map))
}

fn translate_tool_choice(
    choice: Option<&Value>,
    tool_map: &BTreeMap<String, ToolKind>,
) -> BridgeResult<Option<Value>> {
    let Some(choice) = choice.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    if let Some(choice) = choice.as_str() {
        if matches!(choice, "auto" | "none" | "required") {
            return Ok(Some(Value::String(choice.into())));
        }
    }
    if let Some(object) = choice.as_object() {
        if object.get("type").and_then(Value::as_str) == Some("allowed_tools") {
            return Ok(Some(Value::String(
                if object.get("mode").and_then(Value::as_str) == Some("required") {
                    "required"
                } else {
                    "auto"
                }
                .into(),
            )));
        }
        let name = object
            .get("name")
            .or_else(|| object.get("function").and_then(|value| value.get("name")))
            .and_then(non_empty_string);
        if let Some(name) = name {
            if !tool_map.is_empty() && !tool_map.contains_key(name) {
                return Err(BridgeError::new(format!(
                    "tool_choice refers to an unknown tool: {name}."
                ))
                .param("tool_choice"));
            }
            return Ok(Some(json!({
                "type": "function",
                "function": { "name": name }
            })));
        }
    }
    Err(BridgeError::new("Unsupported tool_choice value.")
        .param("tool_choice")
        .code("unsupported_parameter"))
}

fn translate_reasoning_effort(value: Option<&Value>) -> BridgeResult<Option<&'static str>> {
    let Some(effort) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let effort = effort.as_str().unwrap_or("unknown");
    match effort {
        "max" | "ultra" | "xhigh" => Ok(Some("max")),
        "high" | "medium" => Ok(Some("high")),
        "low" | "minimal" | "minimum" | "light" | "none" => Ok(Some("low")),
        _ => Err(
            BridgeError::new(format!("Unsupported reasoning effort: {effort}."))
                .param("reasoning.effort"),
        ),
    }
}

fn translate_response_format(format: Option<&Value>) -> BridgeResult<Option<Value>> {
    let Some(format) = format.and_then(Value::as_object) else {
        return Ok(None);
    };
    match format.get("type").and_then(Value::as_str).unwrap_or("text") {
        "text" => Ok(None),
        "json_object" => Ok(Some(json!({ "type": "json_object" }))),
        "json_schema" => {
            let mut schema = Map::new();
            schema.insert(
                "name".into(),
                format
                    .get("name")
                    .cloned()
                    .unwrap_or_else(|| Value::String("response".into())),
            );
            schema.insert(
                "schema".into(),
                format.get("schema").cloned().unwrap_or_else(|| json!({})),
            );
            if let Some(strict) = format.get("strict").and_then(Value::as_bool) {
                schema.insert("strict".into(), Value::Bool(strict));
            }
            if let Some(description) = format.get("description") {
                schema.insert("description".into(), description.clone());
            }
            Ok(Some(
                json!({ "type": "json_schema", "json_schema": schema }),
            ))
        }
        other => Err(
            BridgeError::new(format!("Unsupported text.format type: {other}."))
                .param("text.format"),
        ),
    }
}

fn derive_prompt_cache_key(input: &Value, model: &str, messages: &[Value]) -> String {
    if let Some(explicit) = input.get("prompt_cache_key").and_then(non_empty_string) {
        return explicit.into();
    }
    for candidate in [
        input
            .get("metadata")
            .and_then(|value| value.get("session_id")),
        input.get("metadata").and_then(|value| value.get("task_id")),
        input
            .get("metadata")
            .and_then(|value| value.get("thread_id")),
        input.get("user"),
    ] {
        if let Some(candidate) = candidate.and_then(non_empty_string) {
            return format!("codex_{}", &hash(candidate)[..40]);
        }
    }
    let stable_prefix: Vec<Value> = messages
        .iter()
        .take(2)
        .map(|message| {
            json!({
                "role": message.get("role").cloned().unwrap_or(Value::Null),
                "content": message.get("content").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    let source = serde_json::to_string(&json!({
        "model": model,
        "stablePrefix": stable_prefix,
    }))
    .unwrap_or_default();
    format!("codex_{}", &hash(&source)[..40])
}

fn append_assistant_tool_call(
    messages: &mut Vec<Value>,
    tool_call: Value,
    reasoning_content: Option<String>,
) {
    if let Some(previous) = messages.last_mut().and_then(Value::as_object_mut) {
        let can_merge = previous.get("role").and_then(Value::as_str) == Some("assistant")
            && previous
                .get("tool_calls")
                .and_then(Value::as_array)
                .is_some()
            && previous
                .get("content")
                .is_none_or(|value| value.is_null() || value.as_str() == Some(""));
        if can_merge {
            previous
                .get_mut("tool_calls")
                .and_then(Value::as_array_mut)
                .expect("tool_calls checked above")
                .push(tool_call);
            if previous
                .get("reasoning_content")
                .and_then(non_empty_string)
                .is_none()
            {
                if let Some(reasoning) = reasoning_content.filter(|value| !value.trim().is_empty())
                {
                    previous.insert("reasoning_content".into(), Value::String(reasoning));
                }
            }
            return;
        }
    }
    let mut message = json!({
        "role": "assistant",
        "content": Value::Null,
        "tool_calls": [tool_call],
    });
    if let Some(reasoning) = reasoning_content.filter(|value| !value.trim().is_empty()) {
        message
            .as_object_mut()
            .expect("json object")
            .insert("reasoning_content".into(), Value::String(reasoning));
    }
    messages.push(message);
}

fn remember_reasoning_for_tool_calls(
    store: Option<&ReasoningStore>,
    tool_calls: &[Value],
    reasoning: &str,
) {
    let Some(store) = store.filter(|_| !reasoning.trim().is_empty()) else {
        return;
    };
    for call in tool_calls {
        if let Some(id) = call.get("id").and_then(non_empty_string) {
            store.set(id, reasoning);
        }
    }
}

fn make_completed_message(text: &str, id: Option<String>) -> Value {
    json!({
        "id": id.unwrap_or_else(|| make_id("msg")),
        "type": "message",
        "status": "completed",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": text,
            "annotations": [],
            "logprobs": [],
        }]
    })
}

fn make_completed_tool_call(tool_call: &Value, tool_map: &BTreeMap<String, ToolKind>) -> Value {
    let name = tool_call
        .get("function")
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let kind = tool_map.get(name).copied().unwrap_or(ToolKind::Function);
    let call_id = tool_call
        .get("id")
        .and_then(non_empty_string)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| make_id("call"));
    let arguments = tool_call
        .get("function")
        .and_then(|value| value.get("arguments"))
        .and_then(Value::as_str)
        .unwrap_or("{}");
    match kind {
        ToolKind::Custom => json!({
            "id": make_id("ctc"),
            "type": "custom_tool_call",
            "status": "completed",
            "call_id": call_id,
            "name": name,
            "input": extract_custom_input(arguments),
        }),
        ToolKind::Function => json!({
            "id": make_id("fc"),
            "type": "function_call",
            "status": "completed",
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
        }),
    }
}

fn make_response_object(
    id: String,
    created_at: Option<i64>,
    model: &str,
    status: &str,
    output: Vec<Value>,
    usage: Option<Value>,
    incomplete_reason: Option<&str>,
    original_request: &Value,
) -> Value {
    json!({
        "id": id,
        "object": "response",
        "created_at": created_at.unwrap_or_else(now_seconds),
        "status": status,
        "background": false,
        "error": Value::Null,
        "incomplete_details": incomplete_reason.map(|reason| json!({ "reason": reason })),
        "instructions": original_request.get("instructions").cloned().unwrap_or(Value::Null),
        "max_output_tokens": original_request.get("max_output_tokens").cloned().unwrap_or(Value::Null),
        "model": model,
        "output": output,
        "parallel_tool_calls": original_request.get("parallel_tool_calls").filter(|value| !value.is_null()).cloned().unwrap_or(Value::Bool(true)),
        "previous_response_id": Value::Null,
        "prompt_cache_key": original_request.get("prompt_cache_key").cloned().unwrap_or(Value::Null),
        "reasoning": original_request.get("reasoning").cloned().unwrap_or(Value::Null),
        "safety_identifier": original_request.get("safety_identifier").cloned().unwrap_or(Value::Null),
        "service_tier": "default",
        "store": false,
        "temperature": original_request.get("temperature").cloned().unwrap_or(Value::Null),
        "text": original_request.get("text").cloned().unwrap_or_else(|| json!({ "format": { "type": "text" } })),
        "tool_choice": original_request.get("tool_choice").cloned().unwrap_or_else(|| Value::String("auto".into())),
        "tools": original_request.get("tools").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        "top_p": original_request.get("top_p").cloned().unwrap_or(Value::Null),
        "truncation": original_request.get("truncation").cloned().unwrap_or_else(|| Value::String("disabled".into())),
        "usage": usage.unwrap_or(Value::Null),
        "user": original_request.get("user").cloned().unwrap_or(Value::Null),
        "metadata": original_request.get("metadata").cloned().unwrap_or_else(|| json!({})),
    })
}

fn normalize_usage(usage: Option<&Value>) -> Value {
    let input = usage
        .and_then(|value| {
            value
                .get("prompt_tokens")
                .or_else(|| value.get("input_tokens"))
        })
        .and_then(non_negative_integer)
        .unwrap_or(0);
    let output = usage
        .and_then(|value| {
            value
                .get("completion_tokens")
                .or_else(|| value.get("output_tokens"))
        })
        .and_then(non_negative_integer)
        .unwrap_or(0);
    let cached = usage
        .and_then(|value| {
            value.get("cached_tokens").or_else(|| {
                value
                    .get("prompt_tokens_details")
                    .and_then(|details| details.get("cached_tokens"))
            })
        })
        .and_then(non_negative_integer)
        .unwrap_or(0);
    let reasoning = usage
        .and_then(|value| {
            value
                .get("completion_tokens_details")
                .and_then(|details| details.get("reasoning_tokens"))
                .or_else(|| {
                    value
                        .get("output_tokens_details")
                        .and_then(|details| details.get("reasoning_tokens"))
                })
        })
        .and_then(non_negative_integer)
        .unwrap_or(0);
    let total = usage
        .and_then(|value| value.get("total_tokens"))
        .and_then(non_negative_integer)
        .unwrap_or(input + output);
    json!({
        "input_tokens": input,
        "input_tokens_details": { "cached_tokens": cached },
        "output_tokens": output,
        "output_tokens_details": { "reasoning_tokens": reasoning },
        "total_tokens": total,
    })
}

#[derive(Debug)]
struct MessageState {
    output_index: usize,
    item_id: String,
    text: String,
}

#[derive(Debug, Clone)]
struct ToolState {
    output_index: usize,
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
    kind: ToolKind,
    added: bool,
}

pub struct StreamTranslator {
    context: TranslationContext,
    response_id: Option<String>,
    chat_id: Option<String>,
    created_at: Option<i64>,
    model: String,
    sequence: u64,
    output: Vec<Option<Value>>,
    next_output_index: usize,
    message: Option<MessageState>,
    tool_calls: BTreeMap<i64, ToolState>,
    usage: Value,
    finish_reason: Option<String>,
    created_emitted: bool,
    reasoning_content: String,
}

impl StreamTranslator {
    pub fn new(context: TranslationContext) -> Self {
        Self {
            model: context.model.clone(),
            context,
            response_id: None,
            chat_id: None,
            created_at: None,
            sequence: 0,
            output: Vec::new(),
            next_output_index: 0,
            message: None,
            tool_calls: BTreeMap::new(),
            usage: normalize_usage(None),
            finish_reason: None,
            created_emitted: false,
            reasoning_content: String::new(),
        }
    }

    pub fn ingest(&mut self, chunk: &Value) -> BridgeResult<Vec<Value>> {
        if let Some(error) = chunk.get("error") {
            return Err(BridgeError::new(
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("The upstream provider returned a streaming error."),
            )
            .status(502)
            .kind(
                error
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("upstream_provider_error"),
            )
            .code(
                error
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("upstream_provider_error"),
            ));
        }
        self.initialize(chunk);
        let mut events = Vec::new();
        if !self.created_emitted {
            self.created_emitted = true;
            let response = self.current_response("in_progress", Vec::new(), None, None);
            events.push(self.event("response.created", json!({ "response": response })));
            let response = self.current_response("in_progress", Vec::new(), None, None);
            events.push(self.event("response.in_progress", json!({ "response": response })));
        }
        if let Some(usage) = chunk.get("usage") {
            self.usage = normalize_usage(Some(usage));
        }
        let Some(choice) = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        else {
            return Ok(events);
        };
        if let Some(usage) = choice.get("usage") {
            self.usage = normalize_usage(Some(usage));
        }
        let delta = choice.get("delta").cloned().unwrap_or_else(|| json!({}));
        if let Some(reasoning) = delta.get("reasoning_content").and_then(Value::as_str) {
            self.reasoning_content.push_str(reasoning);
        }
        if let Some(content) = delta
            .get("content")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        {
            if self.message.is_none() {
                let output_index = self.allocate_output();
                let item_id = make_id("msg");
                self.message = Some(MessageState {
                    output_index,
                    item_id: item_id.clone(),
                    text: String::new(),
                });
                events.push(self.event(
                    "response.output_item.added",
                    json!({
                        "output_index": output_index,
                        "item": {
                            "id": item_id,
                            "type": "message",
                            "status": "in_progress",
                            "role": "assistant",
                            "content": [],
                        }
                    }),
                ));
                events.push(self.event(
                    "response.content_part.added",
                    json!({
                        "item_id": self.message.as_ref().expect("message just set").item_id,
                        "output_index": output_index,
                        "content_index": 0,
                        "part": { "type": "output_text", "text": "", "annotations": [], "logprobs": [] }
                    }),
                ));
            }
            let (item_id, output_index) = {
                let message = self.message.as_mut().expect("message initialized");
                message.text.push_str(content);
                (message.item_id.clone(), message.output_index)
            };
            events.push(self.event(
                "response.output_text.delta",
                json!({
                    "item_id": item_id,
                    "output_index": output_index,
                    "content_index": 0,
                    "delta": content,
                    "logprobs": [],
                }),
            ));
        }
        if let Some(tool_deltas) = delta.get("tool_calls").and_then(Value::as_array) {
            for tool_delta in tool_deltas {
                let chat_index = tool_delta.get("index").and_then(Value::as_i64).unwrap_or(0);
                self.ensure_tool_state(chat_index, tool_delta);
                let mut added_event = None;
                {
                    let state = self
                        .tool_calls
                        .get_mut(&chat_index)
                        .expect("tool state exists");
                    if !state.added && !state.name.is_empty() {
                        state.added = true;
                        added_event = Some((state.output_index, tool_in_progress_item(state)));
                    }
                }
                if let Some((output_index, item)) = added_event {
                    events.push(self.event(
                        "response.output_item.added",
                        json!({ "output_index": output_index, "item": item }),
                    ));
                }
                if let Some(arguments) = tool_delta
                    .get("function")
                    .and_then(|function| function.get("arguments"))
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                {
                    let (kind, added, item_id, output_index) = {
                        let state = self
                            .tool_calls
                            .get_mut(&chat_index)
                            .expect("tool state exists");
                        state.arguments.push_str(arguments);
                        (
                            state.kind,
                            state.added,
                            state.item_id.clone(),
                            state.output_index,
                        )
                    };
                    if kind == ToolKind::Function && added {
                        events.push(self.event(
                            "response.function_call_arguments.delta",
                            json!({
                                "item_id": item_id,
                                "output_index": output_index,
                                "delta": arguments,
                            }),
                        ));
                    }
                }
            }
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.into());
        }
        Ok(events)
    }

    pub fn finish(&mut self) -> Vec<Value> {
        let mut events = Vec::new();
        if !self.created_emitted {
            self.initialize(&json!({}));
            self.created_emitted = true;
            let response = self.current_response("in_progress", Vec::new(), None, None);
            events.push(self.event("response.created", json!({ "response": response })));
        }
        if let Some(message) = self.message.take() {
            let completed = make_completed_message(&message.text, Some(message.item_id.clone()));
            self.output[message.output_index] = Some(completed.clone());
            events.push(self.event(
                "response.output_text.done",
                json!({
                    "item_id": message.item_id,
                    "output_index": message.output_index,
                    "content_index": 0,
                    "text": message.text,
                    "logprobs": [],
                }),
            ));
            events.push(self.event(
                "response.content_part.done",
                json!({
                    "item_id": completed.get("id").cloned().unwrap_or(Value::Null),
                    "output_index": message.output_index,
                    "content_index": 0,
                    "part": completed.get("content").and_then(Value::as_array).and_then(|parts| parts.first()).cloned().unwrap_or(Value::Null),
                }),
            ));
            events.push(self.event(
                "response.output_item.done",
                json!({ "output_index": message.output_index, "item": completed }),
            ));
        }
        let mut tools: Vec<ToolState> = self.tool_calls.values().cloned().collect();
        tools.sort_by_key(|state| state.output_index);
        for mut state in tools {
            if !state.added {
                state.added = true;
                events.push(self.event(
                    "response.output_item.added",
                    json!({
                        "output_index": state.output_index,
                        "item": tool_in_progress_item(&state),
                    }),
                ));
            }
            let completed = completed_tool_state_item(&state);
            self.output[state.output_index] = Some(completed.clone());
            if state.kind == ToolKind::Custom {
                let input = completed.get("input").and_then(Value::as_str).unwrap_or("");
                if !input.is_empty() {
                    events.push(self.event(
                        "response.custom_tool_call_input.delta",
                        json!({
                            "item_id": state.item_id,
                            "output_index": state.output_index,
                            "delta": input,
                        }),
                    ));
                }
                events.push(self.event(
                    "response.custom_tool_call_input.done",
                    json!({
                        "item_id": state.item_id,
                        "output_index": state.output_index,
                        "input": input,
                    }),
                ));
            } else {
                events.push(self.event(
                    "response.function_call_arguments.done",
                    json!({
                        "item_id": state.item_id,
                        "output_index": state.output_index,
                        "name": state.name,
                        "arguments": completed.get("arguments").cloned().unwrap_or_else(|| Value::String("{}".into())),
                    }),
                ));
            }
            events.push(self.event(
                "response.output_item.done",
                json!({ "output_index": state.output_index, "item": completed }),
            ));
        }
        if let Some(store) = self.context.reasoning_store.as_deref() {
            if !self.reasoning_content.trim().is_empty() {
                for state in self.tool_calls.values() {
                    store.set(&state.call_id, &self.reasoning_content);
                }
            }
        }
        let incomplete = self.finish_reason.as_deref() == Some("length");
        let output = self.output.iter().filter_map(Clone::clone).collect();
        let response = self.current_response(
            if incomplete {
                "incomplete"
            } else {
                "completed"
            },
            output,
            Some(self.usage.clone()),
            incomplete.then_some("max_output_tokens"),
        );
        events.push(self.event(
            if incomplete {
                "response.incomplete"
            } else {
                "response.completed"
            },
            json!({ "response": response }),
        ));
        events
    }

    fn initialize(&mut self, chunk: &Value) {
        if self.chat_id.is_none() {
            self.chat_id = Some(
                chunk
                    .get("id")
                    .and_then(non_empty_string)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| make_id("chatcmpl")),
            );
        }
        if self.response_id.is_none() {
            self.response_id = Some(response_id_from(self.chat_id.as_deref()));
        }
        if self.created_at.is_none() {
            self.created_at = Some(
                chunk
                    .get("created")
                    .and_then(Value::as_i64)
                    .unwrap_or_else(now_seconds),
            );
        }
        if let Some(model) = chunk.get("model").and_then(non_empty_string) {
            self.model = model.into();
        }
    }

    fn allocate_output(&mut self) -> usize {
        let index = self.next_output_index;
        self.next_output_index += 1;
        self.output.push(None);
        index
    }

    fn ensure_tool_state(&mut self, chat_index: i64, delta: &Value) {
        if !self.tool_calls.contains_key(&chat_index) {
            let name = delta
                .get("function")
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let kind = self
                .context
                .tool_map
                .get(&name)
                .copied()
                .unwrap_or(ToolKind::Function);
            let output_index = self.allocate_output();
            self.tool_calls.insert(
                chat_index,
                ToolState {
                    output_index,
                    item_id: make_id(if kind == ToolKind::Custom {
                        "ctc"
                    } else {
                        "fc"
                    }),
                    call_id: delta
                        .get("id")
                        .and_then(non_empty_string)
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| make_id("call")),
                    name,
                    arguments: String::new(),
                    kind,
                    added: false,
                },
            );
            return;
        }
        let state = self.tool_calls.get_mut(&chat_index).expect("checked above");
        if let Some(id) = delta.get("id").and_then(non_empty_string) {
            state.call_id = id.into();
        }
        if let Some(name) = delta
            .get("function")
            .and_then(|value| value.get("name"))
            .and_then(Value::as_str)
        {
            state.name.push_str(name);
            if let Some(kind) = self.context.tool_map.get(&state.name) {
                state.kind = *kind;
            }
        }
    }

    fn current_response(
        &self,
        status: &str,
        output: Vec<Value>,
        usage: Option<Value>,
        incomplete_reason: Option<&str>,
    ) -> Value {
        make_response_object(
            self.response_id.clone().unwrap_or_else(|| make_id("resp")),
            self.created_at,
            &self.model,
            status,
            output,
            usage,
            incomplete_reason,
            &self.context.original_request,
        )
    }

    fn event(&mut self, kind: &str, fields: Value) -> Value {
        let mut event = fields.as_object().cloned().unwrap_or_default();
        event.insert("type".into(), Value::String(kind.into()));
        event.insert(
            "sequence_number".into(),
            Value::Number(self.sequence.into()),
        );
        self.sequence += 1;
        Value::Object(event)
    }
}

fn tool_in_progress_item(state: &ToolState) -> Value {
    match state.kind {
        ToolKind::Custom => json!({
            "id": state.item_id,
            "type": "custom_tool_call",
            "status": "in_progress",
            "call_id": state.call_id,
            "name": state.name,
            "input": "",
        }),
        ToolKind::Function => json!({
            "id": state.item_id,
            "type": "function_call",
            "status": "in_progress",
            "call_id": state.call_id,
            "name": state.name,
            "arguments": "",
        }),
    }
}

fn completed_tool_state_item(state: &ToolState) -> Value {
    match state.kind {
        ToolKind::Custom => json!({
            "id": state.item_id,
            "type": "custom_tool_call",
            "status": "completed",
            "call_id": state.call_id,
            "name": state.name,
            "input": extract_custom_input(&state.arguments),
        }),
        ToolKind::Function => json!({
            "id": state.item_id,
            "type": "function_call",
            "status": "completed",
            "call_id": state.call_id,
            "name": state.name,
            "arguments": if state.arguments.is_empty() { "{}" } else { &state.arguments },
        }),
    }
}

fn coerce_instruction_text(value: &Value) -> BridgeResult<String> {
    if let Some(text) = value.as_str() {
        return Ok(text.into());
    }
    if let Some(parts) = value.as_array() {
        return Ok(parts
            .iter()
            .map(|part| {
                part.as_str()
                    .or_else(|| part.get("text").and_then(Value::as_str))
                    .unwrap_or("")
            })
            .collect());
    }
    Err(BridgeError::new("instructions must be a string or text-part array.").param("instructions"))
}

fn coerce_tool_output(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        None | Some(Value::Null) => String::new(),
        Some(value) => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn stringify_arguments(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(value) => serde_json::to_string(value).unwrap_or_else(|_| "{}".into()),
        None => "{}".into(),
    }
}

fn normalize_assistant_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .map(|part| {
                part.as_str()
                    .or_else(|| part.get("text").and_then(Value::as_str))
                    .unwrap_or("")
            })
            .collect(),
        _ => String::new(),
    }
}

fn extract_custom_input(arguments: &str) -> String {
    if arguments.is_empty() {
        return String::new();
    }
    serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| {
            value
                .get("input")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| arguments.into())
}

fn response_id_from(id: Option<&str>) -> String {
    if let Some(id) = id.filter(|id| id.starts_with("resp_")) {
        return id.into();
    }
    let suffix = id
        .map(|id| {
            id.chars()
                .filter(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
                .collect::<String>()
        })
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| make_id("r"));
    format!("resp_{suffix}")
}

fn make_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn non_empty_string(value: &Value) -> Option<&str> {
    value.as_str().and_then(non_empty_text)
}

fn non_empty_text(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then_some(value)
}

fn positive_integer(value: &Value) -> Option<u64> {
    non_negative_integer(value).filter(|value| *value > 0)
}

fn non_negative_integer(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_i64()
            .and_then(|number| (number >= 0).then_some(number as u64))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_responses_request() {
        let translated = translate_responses_request(
            json!({
                "model": "k3",
                "instructions": "Review only. Do not edit.",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [
                        { "type": "input_text", "text": "Inspect this screenshot." },
                        { "type": "input_image", "image_url": "data:image/png;base64,AAAA", "detail": "high" }
                    ]
                }],
                "tools": [
                    {
                        "type": "function",
                        "name": "read_file",
                        "description": "Read a file",
                        "parameters": { "type": "object", "properties": { "path": { "type": "string" } } },
                        "strict": true
                    },
                    { "type": "custom", "name": "apply_patch", "description": "Apply a patch" }
                ],
                "reasoning": { "effort": "xhigh" },
                "max_output_tokens": 4096,
                "parallel_tool_calls": true,
                "stream": true
            }),
            "k3",
            None,
        )
        .unwrap();
        assert_eq!(translated.body["model"], "k3");
        assert_eq!(translated.body["messages"][0]["role"], "system");
        assert_eq!(
            translated.body["messages"][1]["content"][1]["type"],
            "image_url"
        );
        assert_eq!(translated.body["reasoning_effort"], "max");
        assert_eq!(translated.body["max_completion_tokens"], 4096);
        assert!(translated.body.get("parallel_tool_calls").is_none());
        assert_eq!(translated.context.tool_map["apply_patch"], ToolKind::Custom);
        assert!(
            translated.body["prompt_cache_key"]
                .as_str()
                .unwrap()
                .starts_with("codex_")
        );
    }

    #[test]
    fn preserves_reasoning_across_tool_history() {
        let store = Arc::new(ReasoningStore::new());
        store.set("call_1", "private preserved reasoning");
        let translated = translate_responses_request(
            json!({
                "model": "k3",
                "input": [
                    { "role": "user", "content": "Read package.json" },
                    { "type": "function_call", "call_id": "call_1", "name": "read_file", "arguments": "{\"path\":\"package.json\"}" },
                    { "type": "function_call_output", "call_id": "call_1", "output": "{\"name\":\"demo\"}" }
                ],
                "stream": false
            }),
            "k3",
            Some(store),
        )
        .unwrap();
        assert_eq!(
            translated.body["messages"][1]["reasoning_content"],
            "private preserved reasoning"
        );
        assert_eq!(translated.body["messages"][2]["role"], "tool");
    }

    #[test]
    fn rejects_unsupported_tools() {
        let error = translate_responses_request(
            json!({ "model": "k3", "input": "Search", "tools": [{ "type": "web_search_preview" }] }),
            "k3",
            None,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "unsupported_tool_type");
    }

    #[test]
    fn translates_non_streaming_completion() {
        let store = Arc::new(ReasoningStore::new());
        let request = translate_responses_request(
            json!({
                "model": "k3",
                "input": "Use the tool",
                "tools": [{ "type": "function", "name": "read_file", "parameters": { "type": "object" } }],
                "stream": false
            }),
            "k3",
            Some(store.clone()),
        )
        .unwrap();
        let response = translate_chat_completion(
            &json!({
                "id": "chatcmpl_123",
                "created": 123,
                "model": "k3",
                "choices": [{
                    "finish_reason": "tool_calls",
                    "message": {
                        "role": "assistant",
                        "content": "I will inspect it.",
                        "reasoning_content": "reason before tool use",
                        "tool_calls": [{ "id": "call_abc", "type": "function", "function": { "name": "read_file", "arguments": "{\"path\":\"a\"}" } }]
                    }
                }],
                "usage": { "prompt_tokens": 12, "completion_tokens": 5, "total_tokens": 17, "cached_tokens": 3 }
            }),
            &request.context,
        )
        .unwrap();
        assert_eq!(response["output"][1]["type"], "function_call");
        assert_eq!(
            response["usage"]["input_tokens_details"]["cached_tokens"],
            3
        );
        assert_eq!(
            store.get("call_abc").as_deref(),
            Some("reason before tool use")
        );
    }

    #[test]
    fn streams_text_and_tools() {
        let store = Arc::new(ReasoningStore::new());
        let request = translate_responses_request(
            json!({
                "model": "k3",
                "input": "Use tools",
                "tools": [
                    { "type": "function", "name": "read_file", "parameters": { "type": "object" } },
                    { "type": "custom", "name": "apply_patch" }
                ],
                "stream": true
            }),
            "k3",
            Some(store.clone()),
        )
        .unwrap();
        let mut stream = StreamTranslator::new(request.context);
        let mut events = stream.ingest(&json!({
            "id": "chatcmpl_tools",
            "created": 10,
            "model": "k3",
            "choices": [{
                "delta": {
                    "content": "Hello",
                    "reasoning_content": "reason ",
                    "tool_calls": [
                        { "index": 0, "id": "call_read", "function": { "name": "read_file", "arguments": "{\"path\":" } },
                        { "index": 1, "id": "call_patch", "function": { "name": "apply_patch", "arguments": "{\"input\":\"***" } }
                    ]
                },
                "finish_reason": null
            }]
        })).unwrap();
        events.extend(
            stream
                .ingest(&json!({
                    "id": "chatcmpl_tools",
                    "choices": [{
                        "delta": {
                            "content": " world",
                            "reasoning_content": "continued",
                            "tool_calls": [
                                { "index": 0, "function": { "arguments": "\"a.txt\"}" } },
                                { "index": 1, "function": { "arguments": " Begin Patch\"}" } }
                            ]
                        },
                        "finish_reason": "tool_calls"
                    }]
                }))
                .unwrap(),
        );
        events.extend(stream.finish());
        assert!(
            events
                .iter()
                .any(|event| event["type"] == "response.output_text.delta")
        );
        let completed = events.last().unwrap();
        assert_eq!(completed["type"], "response.completed");
        assert_eq!(
            completed["response"]["output"][0]["content"][0]["text"],
            "Hello world"
        );
        assert_eq!(
            completed["response"]["output"][1]["arguments"],
            "{\"path\":\"a.txt\"}"
        );
        assert_eq!(
            completed["response"]["output"][2]["input"],
            "*** Begin Patch"
        );
        assert_eq!(store.get("call_read").as_deref(), Some("reason continued"));
    }
}
