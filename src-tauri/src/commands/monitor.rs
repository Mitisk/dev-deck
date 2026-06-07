use crate::error::AppResult;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResult {
    pub ok: bool,        // 2xx
    pub reachable: bool, // получили любой HTTP-ответ
    pub status: u16,     // HTTP-код (0 при сетевой ошибке)
    pub latency_ms: u64,
    pub detail: Option<String>, // status из JSON или текст ошибки
}

/// Проверить доступность URL (health-чек проекта). Сетевые ошибки не падают,
/// а возвращаются как reachable=false (тостов не будет).
#[tauri::command]
pub fn health_check(url: String) -> AppResult<HealthResult> {
    let u = url.trim();
    let u = if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else {
        format!("https://{}", u)
    };
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(6))
        .build();
    let start = Instant::now();
    match agent.get(&u).call() {
        Ok(resp) => {
            let status = resp.status();
            let latency_ms = start.elapsed().as_millis() as u64;
            let detail = parse_json_status(resp);
            Ok(HealthResult { ok: (200..300).contains(&status), reachable: true, status, latency_ms, detail })
        }
        Err(ureq::Error::Status(code, _resp)) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(HealthResult { ok: false, reachable: true, status: code, latency_ms, detail: None })
        }
        Err(ureq::Error::Transport(_)) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(HealthResult { ok: false, reachable: false, status: 0, latency_ms, detail: Some("сервис недоступен".into()) })
        }
    }
}

/// Если ответ — JSON, вернуть значение поля status/state/health.
fn parse_json_status(resp: ureq::Response) -> Option<String> {
    if !resp.content_type().contains("json") {
        return None;
    }
    let body = resp.into_string().ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    for key in ["status", "state", "health"] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
    }
    None
}
