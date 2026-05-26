use serde_json::{json, Value};
use std::collections::BTreeMap;

/// 把上游 SSE 中一条 `delta.tool_calls` 数组累加到 tool_calls 汇总表。
///
/// 上游对同一个 tool_call 会按 index 分片返回 name / arguments，
/// 这里按 index 聚合，最终在流结束时转成 `Vec<Value>`。
pub fn accumulate_tool_call_deltas(acc: &mut BTreeMap<usize, Value>, tool_deltas: &[Value]) {
    for tc_delta in tool_deltas {
        let idx = tc_delta.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let entry = acc.entry(idx).or_insert_with(|| {
            json!({
                "id": "",
                "type": "function",
                "function": {
                    "name": "",
                    "arguments": ""
                }
            })
        });
        if let Some(id) = tc_delta.get("id").and_then(|v| v.as_str()) {
            entry["id"] = Value::String(id.to_string());
        }
        if let Some(typ) = tc_delta.get("type").and_then(|v| v.as_str()) {
            entry["type"] = Value::String(typ.to_string());
        }
        if let Some(function) = tc_delta.get("function").and_then(|v| v.as_object()) {
            if let Some(name) = function.get("name").and_then(|v| v.as_str()) {
                entry["function"]["name"] = Value::String(name.to_string());
            }
            if let Some(arguments) = function.get("arguments").and_then(|v| v.as_str()) {
                let current = entry["function"]["arguments"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                entry["function"]["arguments"] = Value::String(format!("{current}{arguments}"));
            }
        }
    }
}

/// 解析单个 tool_call 的 (name, args)。
pub fn parse_tool_call(tool_call: &Value) -> (String, Value) {
    let name = tool_call
        .get("function")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let args = tool_call
        .get("function")
        .and_then(|v| v.get("arguments"))
        .and_then(|v| v.as_str())
        .and_then(|v| serde_json::from_str::<Value>(v).ok())
        .unwrap_or_else(|| json!({}));
    (name, args)
}

/// 获取 tool_call_id 字符串。
pub fn tool_call_id(tool_call: &Value) -> String {
    tool_call
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// 从模型输出文本中提取 prompt 注入版工具调用。
///
/// 约定格式：
/// `<tool_call>{"name":"get_weather","arguments":{"city":"上海"}}</tool_call>`
pub fn extract_prompt_tool_calls(rendered: &str) -> (Vec<Value>, String) {
    let mut tool_calls = Vec::new();
    let mut visible = String::new();
    let mut rest = rendered;
    let start_tag = "<tool_call>";
    let end_tag = "</tool_call>";
    let mut idx = 0usize;

    loop {
        let Some(start) = rest.find(start_tag) else {
            visible.push_str(rest);
            break;
        };
        visible.push_str(&rest[..start]);
        let after_start = &rest[start + start_tag.len()..];
        let Some(end) = after_start.find(end_tag) else {
            visible.push_str(&rest[start..]);
            break;
        };
        let raw = after_start[..end].trim();
        if let Ok(value) = serde_json::from_str::<Value>(raw) {
            let name = value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let arguments = value.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if !name.is_empty() {
                tool_calls.push(json!({
                    "id": format!("react_call_{}", idx),
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_string())
                    }
                }));
                idx += 1;
            }
        }
        rest = &after_start[end + end_tag.len()..];
    }

    (tool_calls, visible.trim().to_string())
}
