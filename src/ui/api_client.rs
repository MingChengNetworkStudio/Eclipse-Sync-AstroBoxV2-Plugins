//! HTTP 执行层：插件自身调用铭诚 API、供应商 chat/completions（含 SSE 流式）。
//! 普通请求一次性读取响应体；SSE 请求以流式方式逐事件回调。

use std::collections::HashMap;
use std::io::Read;

use flate2::read::{DeflateDecoder, GzDecoder};
use url::Url;
use waki::bindings::wasi::http::{outgoing_handler, types as http_types};
use waki::bindings::wasi::io::streams::StreamError;

/// 普通 HTTP 响应结果
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// 执行普通 HTTP 请求（一次性读取完整响应体）
pub fn execute_request(
    method: &str,
    url: &str,
    headers: &HashMap<String, String>,
    body: Option<&[u8]>,
    timeout_ms: u32,
) -> Result<HttpResponse, String> {
    let parsed = Url::parse(url).map_err(|e| format!("URL解析失败: {}", e))?;

    // 带 body 却未声明 Content-Type 时补默认值，避免服务端无法解析请求体
    let mut owned_headers = headers.clone();
    if body.map(|b| !b.is_empty()).unwrap_or(false) {
        let has_ct = owned_headers
            .keys()
            .any(|k| k.eq_ignore_ascii_case("content-type"));
        if !has_ct {
            let is_json = body
                .map(|b| {
                    let first = b.iter().find(|x| !x.is_ascii_whitespace()).copied().unwrap_or(0);
                    first == b'{' || first == b'['
                })
                .unwrap_or(false);
            let ct = if is_json {
                "application/json;charset=UTF-8"
            } else {
                "application/x-www-form-urlencoded"
            };
            owned_headers.insert("Content-Type".to_string(), ct.to_string());
        }
    }

    let (req, outgoing_body) = build_outgoing_request(method, &parsed, &owned_headers, body)?;

    // 发送请求体（仅当有 body 时才打开写入流）
    if let Some(data) = body {
        if !data.is_empty() {
            let stream = outgoing_body
                .write()
                .map_err(|e| format!("打开请求体写入流失败: {:?}", e))?;
            stream
                .blocking_write_and_flush(data)
                .map_err(|e| format!("写入请求体失败: {:?}", e))?;
            drop(stream);
        }
    }
    http_types::OutgoingBody::finish(outgoing_body, None)
        .map_err(|e| format!("结束请求体失败: {:?}", e))?;

    let (incoming_response, status_code) = send_and_wait(req, timeout_ms)?;

    // 收集响应头
    let mut resp_headers = HashMap::new();
    for (k, v) in incoming_response.headers().entries() {
        if let Ok(val) = String::from_utf8(v) {
            resp_headers.insert(k.to_lowercase(), val);
        }
    }

    let raw = read_full_body(incoming_response)?;
    let body = decompress_if_needed(&raw, &resp_headers);

    Ok(HttpResponse {
        status_code,
        headers: resp_headers,
        body,
    })
}

/// SSE 事件回调：收到一个完整事件时调用
///   - event: 事件类型（默认 "message"）
///   - data: 事件数据
/// 返回 false 可中止流
pub type SseCallback<'a> = dyn FnMut(&str, &str) -> bool + 'a;

/// 执行 SSE 流式请求，逐事件回调。`is_cancelled` 在每次读取前检查。
pub fn execute_sse(
    method: &str,
    url: &str,
    headers: &HashMap<String, String>,
    body: Option<&[u8]>,
    timeout_ms: u32,
    on_event: &mut SseCallback<'_>,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|e| format!("URL解析失败: {}", e))?;
    let (req, outgoing_body) = build_outgoing_request(method, &parsed, headers, body)?;

    if let Some(data) = body {
        let stream = outgoing_body
            .write()
            .map_err(|_| "打开请求体写入流失败".to_string())?;
        stream
            .blocking_write_and_flush(data)
            .map_err(|e| format!("写入请求体失败: {:?}", e))?;
        drop(stream);
    }
    http_types::OutgoingBody::finish(outgoing_body, None)
        .map_err(|_| "结束请求体失败".to_string())?;

    let (incoming_response, status_code) = send_and_wait(req, timeout_ms)?;
    if !(200..300).contains(&status_code) {
        return Err(format!("SSE 连接失败，状态码: {}", status_code));
    }

    let incoming_body = incoming_response
        .consume()
        .map_err(|_| "读取响应体失败".to_string())?;
    let input_stream = incoming_body
        .stream()
        .map_err(|_| "打开响应体流失败".to_string())?;

    // 逐块读取并按 SSE 事件边界（空行）分割
    let mut buffer = Vec::new();
    let mut event_type = "message".to_string();

    loop {
        if is_cancelled() {
            return Ok(());
        }

        let chunk = match input_stream.blocking_read(1024) {
            Ok(c) if c.is_empty() => break,
            Ok(c) => c,
            Err(StreamError::Closed) => break,
            Err(e) => return Err(format!("读取 SSE 流失败: {:?}", e)),
        };
        buffer.extend_from_slice(&chunk);

        while let Some(boundary) = find_event_boundary(&buffer) {
            let event_bytes = buffer[..boundary].to_vec();
            buffer = buffer[boundary..].to_vec();

            if let Some((evt, data)) = parse_sse_event(&event_bytes, &event_type) {
                if !on_event(&evt, &data) {
                    return Ok(());
                }
            }
            // 事件类型在事件结束后重置为默认
            event_type = "message".to_string();
        }

        // 在块内解析 "event:" 行以更新事件类型
        if let Some(last_event) = extract_last_event_field(&buffer) {
            event_type = last_event;
        }
    }

    Ok(())
}

// ========== JSON 便捷封装 ==========

/// 发起 GET 请求并解析 JSON 响应（非 2xx 时尝试提取 message 字段作为错误）
pub fn get_json(url: &str, headers: &HashMap<String, String>) -> Result<serde_json::Value, String> {
    let resp = execute_request("GET", url, headers, None, 20000)?;
    parse_json_response(resp.status_code, resp.body)
}

/// 发起 POST JSON 请求，返回 HTTP 状态码与解析后的 JSON（调用方需区分 200/201）
pub fn post_json_with_status(
    url: &str,
    payload: &serde_json::Value,
    extra_headers: &HashMap<String, String>,
) -> Result<(u16, serde_json::Value), String> {
    let body = serde_json::to_vec(payload).map_err(|e| format!("请求序列化失败: {}", e))?;
    let resp = execute_request("POST", url, extra_headers, Some(&body), 20000)?;
    let status = resp.status_code;
    let json = parse_json_response(status, resp.body)?;
    Ok((status, json))
}

/// 解析 JSON 响应体；gzip 已在 execute_request 阶段处理
fn parse_json_response(status: u16, body: Vec<u8>) -> Result<serde_json::Value, String> {
    if body.is_empty() {
        return Err("响应体为空".to_string());
    }
    let json: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| format!("响应解析失败: {}", e))?;

    if (200..300).contains(&status) {
        return Ok(json);
    }
    if let Some(message) = json.get("message").and_then(|v| v.as_str()) {
        return Err(message.to_string());
    }
    if let Some(code) = json.get("code").and_then(|v| v.as_str()) {
        return Err(format!("接口错误: {}", code));
    }
    Err(format!("HTTP {}", status))
}

// ========== WASI HTTP 底层组装 ==========

/// 构建 OutgoingRequest
fn build_outgoing_request(
    method: &str,
    url: &Url,
    headers: &HashMap<String, String>,
    _body: Option<&[u8]>,
) -> Result<(http_types::OutgoingRequest, http_types::OutgoingBody), String> {
    let header_entries: Vec<(String, Vec<u8>)> = headers
        .iter()
        .map(|(k, v)| (k.clone(), v.as_bytes().to_vec()))
        .collect();

    let http_headers = http_types::Headers::from_list(&header_entries)
        .map_err(|e| format!("请求头无效: {:?}", e))?;
    let req = http_types::OutgoingRequest::new(http_headers);

    let http_method = match method.to_ascii_uppercase().as_str() {
        "GET" => http_types::Method::Get,
        "POST" => http_types::Method::Post,
        "PUT" => http_types::Method::Put,
        "DELETE" => http_types::Method::Delete,
        "HEAD" => http_types::Method::Head,
        "PATCH" => http_types::Method::Patch,
        "OPTIONS" => http_types::Method::Options,
        other => http_types::Method::Other(other.to_string()),
    };
    req.set_method(&http_method)
        .map_err(|_| "设置请求方法失败".to_string())?;

    let scheme = match url.scheme() {
        "https" => http_types::Scheme::Https,
        _ => http_types::Scheme::Http,
    };
    req.set_scheme(Some(&scheme))
        .map_err(|_| "设置协议失败".to_string())?;
    req.set_authority(Some(url.authority()))
        .map_err(|_| "设置主机失败".to_string())?;

    let path = match url.query() {
        Some(q) => format!("{}?{}", url.path(), q),
        None => url.path().to_string(),
    };
    req.set_path_with_query(Some(&path))
        .map_err(|_| "设置路径失败".to_string())?;

    let outgoing_body = req
        .body()
        .map_err(|_| "获取请求体失败".to_string())?;

    Ok((req, outgoing_body))
}

/// 发送请求并等待响应（轮询 future），同时应用连接超时
fn send_and_wait(
    req: http_types::OutgoingRequest,
    timeout_ms: u32,
) -> Result<(http_types::IncomingResponse, u16), String> {
    // WASI duration 单位为纳秒
    let timeout_ns = (timeout_ms as u64) * 1_000_000;
    let options = http_types::RequestOptions::new();
    options
        .set_connect_timeout(Some(timeout_ns))
        .map_err(|()| "设置超时失败".to_string())?;
    let future_response = outgoing_handler::handle(req, Some(options))
        .map_err(|e| format!("请求发送失败: {:?}", e))?;

    let incoming_response = match future_response.get() {
        Some(result) => result.map_err(|()| "response already taken".to_string())?,
        None => {
            let pollable = future_response.subscribe();
            pollable.block();
            future_response
                .get()
                .ok_or_else(|| "response not available".to_string())?
                .map_err(|()| "response already taken".to_string())?
        }
    }
    .map_err(|e| format!("响应错误: {:?}", e))?;

    let status_code = incoming_response.status();
    Ok((incoming_response, status_code))
}

/// 一次性读完响应体
fn read_full_body(
    incoming_response: http_types::IncomingResponse,
) -> Result<Vec<u8>, String> {
    let incoming_body = incoming_response
        .consume()
        .map_err(|_| "读取响应体失败".to_string())?;
    let input_stream = incoming_body
        .stream()
        .map_err(|_| "打开响应体流失败".to_string())?;

    let mut raw = Vec::new();
    loop {
        match input_stream.blocking_read(1024 * 64) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break;
                }
                raw.extend_from_slice(&chunk);
            }
            Err(StreamError::Closed) => break,
            Err(e) => return Err(format!("读取响应体失败: {:?}", e)),
        }
    }
    Ok(raw)
}

/// 根据 Content-Encoding / gzip 魔数自动解压
fn decompress_if_needed(body: &[u8], headers: &HashMap<String, String>) -> Vec<u8> {
    let encoding = headers
        .get("content-encoding")
        .map(|s| s.as_str())
        .unwrap_or("");

    if body.len() >= 2 && body[0] == 0x1f && body[1] == 0x8b {
        let mut decoder = GzDecoder::new(body);
        let mut out = Vec::new();
        if decoder.read_to_end(&mut out).is_ok() {
            return out;
        }
    }

    if encoding.contains("deflate") {
        let mut decoder = DeflateDecoder::new(body);
        let mut out = Vec::new();
        if decoder.read_to_end(&mut out).is_ok() {
            return out;
        }
    }

    body.to_vec()
}

/// 在缓冲区中查找 SSE 事件边界（\n\n 或 \r\n\r\n）
fn find_event_boundary(buffer: &[u8]) -> Option<usize> {
    for i in 0..buffer.len().saturating_sub(1) {
        if buffer[i] == b'\n' && buffer[i + 1] == b'\n' {
            return Some(i + 2);
        }
        if buffer[i] == b'\r'
            && i + 3 < buffer.len()
            && buffer[i + 1] == b'\n'
            && buffer[i + 2] == b'\r'
            && buffer[i + 3] == b'\n'
        {
            return Some(i + 4);
        }
    }
    None
}

/// 从缓冲中提取最后一个 "event:" 字段值
fn extract_last_event_field(buffer: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(buffer);
    let mut last_event = None;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("event:") {
            last_event = Some(value.trim().to_string());
        }
    }
    last_event
}

/// 解析单个 SSE 事件块，返回 (event_type, data)
fn parse_sse_event(event_bytes: &[u8], current_event: &str) -> Option<(String, String)> {
    let text = String::from_utf8_lossy(event_bytes);
    let mut data_lines = Vec::new();
    let mut event_type = current_event.to_string();

    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.trim_start().to_string());
        } else if let Some(value) = line.strip_prefix("event:") {
            event_type = value.trim().to_string();
        }
        // 忽略 id:、retry: 等字段
    }

    if data_lines.is_empty() {
        return None;
    }

    Some((event_type, data_lines.join("\n")))
}
