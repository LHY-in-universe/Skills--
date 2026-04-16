use serde_json::{json, Value};
use std::collections::BTreeMap;

/// 把上游 SSE 中一条 `delta.tool_calls` 数组累加到 tool_calls 汇总表。
///
/// 上游对同一个 tool_call 会按 index 分片返回 name / arguments，
/// 这里按 index 聚合，最终在流结束时转成 `Vec<Value>`。
pub fn accumulate_tool_call_deltas(
    acc: &mut BTreeMap<usize, Value>,
    tool_deltas: &[Value],
) {
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
                entry["function"]["arguments"] =
                    Value::String(format!("{current}{arguments}"));
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
