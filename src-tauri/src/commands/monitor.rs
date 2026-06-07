use crate::error::AppResult;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthComponent {
    pub name: String,
    pub state: String, // up | warn | down | unknown
    pub label: String, // исходный текст статуса (как в JSON)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResult {
    pub ok: bool,        // 2xx
    pub reachable: bool, // получили любой HTTP-ответ
    pub status: u16,     // HTTP-код (0 при сетевой ошибке)
    pub latency_ms: u64,
    pub detail: Option<String>, // верхнеуровневый status из JSON или текст ошибки
    pub components: Vec<HealthComponent>, // зависимости (redis/db/cache/...) если есть
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
            let (detail, components) = if resp.content_type().contains("json") {
                resp.into_string().ok().map(|b| parse_body(&b)).unwrap_or((None, Vec::new()))
            } else {
                (None, Vec::new())
            };
            Ok(HealthResult { ok: (200..300).contains(&status), reachable: true, status, latency_ms, detail, components })
        }
        Err(ureq::Error::Status(code, _resp)) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(HealthResult { ok: false, reachable: true, status: code, latency_ms, detail: None, components: Vec::new() })
        }
        Err(ureq::Error::Transport(_)) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Ok(HealthResult { ok: false, reachable: false, status: 0, latency_ms, detail: Some("сервис недоступен".into()), components: Vec::new() })
        }
    }
}

/// Разобрать тело health-ответа: верхний статус + список зависимостей.
/// Понимает Spring Actuator (`components`), NestJS Terminus (`info`/`details`),
/// кастомные (`services`/`dependencies`) и массив `checks`.
fn parse_body(body: &str) -> (Option<String>, Vec<HealthComponent>) {
    let v: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return (None, Vec::new()),
    };
    let detail = top_status(&v);
    let mut comps = Vec::new();
    let mut seen = HashSet::new();
    for key in ["components", "services", "checks", "info", "details", "dependencies"] {
        if let Some(node) = v.get(key) {
            collect(node, &mut comps, &mut seen);
        }
    }
    comps.truncate(24);
    (detail, comps)
}

fn top_status(v: &Value) -> Option<String> {
    for k in ["status", "state", "health"] {
        if let Some(s) = v.get(k).and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
    }
    None
}

/// Достать строковый статус из значения (строка / bool / объект со status / массив проверок).
fn status_of(val: &Value) -> Option<String> {
    match val {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(if *b { "up".into() } else { "down".into() }),
        Value::Object(_) => {
            for k in ["status", "state", "health"] {
                if let Some(s) = val.get(k).and_then(|x| x.as_str()) {
                    return Some(s.to_string());
                }
                if let Some(b) = val.get(k).and_then(|x| x.as_bool()) {
                    return Some(if b { "up".into() } else { "down".into() });
                }
            }
            None
        }
        Value::Array(a) => a.first().and_then(status_of),
        _ => None,
    }
}

fn collect(node: &Value, out: &mut Vec<HealthComponent>, seen: &mut HashSet<String>) {
    match node {
        // объект: имя -> статус/объект/массив
        Value::Object(map) => {
            for (name, val) in map {
                if let Some(st) = status_of(val) {
                    if seen.insert(name.to_lowercase()) {
                        out.push(HealthComponent { name: name.clone(), state: normalize(&st), label: st });
                    }
                }
            }
        }
        // массив: [{name/component/key, status}]
        Value::Array(arr) => {
            for item in arr {
                let name = item
                    .get("name")
                    .or_else(|| item.get("component"))
                    .or_else(|| item.get("key"))
                    .and_then(|x| x.as_str());
                if let (Some(n), Some(st)) = (name, status_of(item)) {
                    if seen.insert(n.to_lowercase()) {
                        out.push(HealthComponent { name: n.to_string(), state: normalize(&st), label: st });
                    }
                }
            }
        }
        _ => {}
    }
}

/// Привести разнообразные статусы к up | warn | down | unknown.
fn normalize(s: &str) -> String {
    let l = s.trim().to_lowercase();
    if ["up", "ok", "pass", "passing", "healthy", "online", "true", "ready", "available", "running", "green"].contains(&l.as_str()) {
        "up".into()
    } else if ["degraded", "warn", "warning", "partial", "slow", "yellow"].contains(&l.as_str()) {
        "warn".into()
    } else if ["down", "fail", "failing", "error", "critical", "unhealthy", "offline", "false", "outage", "unavailable", "stopped", "red"].contains(&l.as_str()) {
        "down".into()
    } else {
        "unknown".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spring_actuator() {
        let body = r#"{"status":"UP","components":{"redis":{"status":"UP"},"db":{"status":"DOWN"},"diskSpace":{"status":"UP","details":{"free":1}}}}"#;
        let (detail, comps) = parse_body(body);
        assert_eq!(detail.as_deref(), Some("UP"));
        let get = |n: &str| comps.iter().find(|c| c.name == n).map(|c| c.state.as_str());
        assert_eq!(get("redis"), Some("up"));
        assert_eq!(get("db"), Some("down"));
        assert_eq!(get("diskSpace"), Some("up"));
    }

    #[test]
    fn parses_custom_services_and_array_checks() {
        let body = r#"{"status":"ok","services":{"cache":"degraded","queue":true},"checks":[{"name":"smtp","status":"pass"}]}"#;
        let (detail, comps) = parse_body(body);
        assert_eq!(detail.as_deref(), Some("ok"));
        let get = |n: &str| comps.iter().find(|c| c.name == n).map(|c| c.state.as_str());
        assert_eq!(get("cache"), Some("warn"));
        assert_eq!(get("queue"), Some("up"));
        assert_eq!(get("smtp"), Some("up"));
    }

    #[test]
    fn plain_status_no_components() {
        let (detail, comps) = parse_body(r#"{"status":"ok"}"#);
        assert_eq!(detail.as_deref(), Some("ok"));
        assert!(comps.is_empty());
    }
}
