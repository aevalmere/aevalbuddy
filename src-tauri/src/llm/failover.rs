//! Moving a request to the next free model when the failure is one another
//! model could answer (PARITY FAIL-02..04, FAIL-06). Port of `withFailover`
//! and `shortModel` in main.js.

use std::future::Future;

use super::errors::{should_failover, ApiError};

/// Two stand-ins ride out the usual free-tier queue and still let a real
/// outage surface quickly.
pub const MAX_MODEL_ATTEMPTS: usize = 3;

/// "google/gemma-4-26b-a4b-it:free" reads as "gemma-4-26b-a4b-it" in a
/// status line; 28 characters at most, with an ellipsis.
pub fn short_model(id: &str) -> String {
    let last = id.rsplit('/').next().unwrap_or_default();
    let name = last.strip_suffix(":free").unwrap_or(last);
    let units: Vec<u16> = name.encode_utf16().collect();
    if units.len() > 28 {
        format!("{}\u{2026}", String::from_utf16_lossy(&units[..27]))
    } else {
        name.to_string()
    }
}

/// Runs `run` on `first`, then on each model `next` offers, while the error
/// is a failover status, the model is a `:free` one (a paid request never
/// changes bill), fewer than three models have been tried, and `can_retry`
/// says nothing has reached the user yet. Returns the model that answered.
pub async fn with_failover<T, F, Fut>(
    first: &str,
    mut next: impl FnMut(&[String]) -> Option<String>,
    can_retry: impl Fn() -> bool,
    mut on_switch: impl FnMut(&str),
    mut run: F,
) -> Result<(String, T), ApiError>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<T, ApiError>>,
{
    let mut tried: Vec<String> = Vec::new();
    let mut attempt = Some(first.to_string());
    let mut last_err = None;
    while let Some(model) = attempt {
        tried.push(model.clone());
        if tried.len() > 1 {
            on_switch(&model);
        }
        match run(model.clone()).await {
            Ok(value) => return Ok((model, value)),
            Err(err) => {
                if err.is_abort() || !can_retry() {
                    return Err(err);
                }
                if !should_failover(&err) || !model.ends_with(":free") {
                    return Err(err);
                }
                attempt = if tried.len() >= MAX_MODEL_ATTEMPTS {
                    None
                } else {
                    next(&tried)
                };
                last_err = Some(err);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| ApiError::other("no model to try")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn block<T>(f: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(f)
    }

    fn pool(tried: &[String]) -> Option<String> {
        ["a:free", "b:free", "c:free", "d:free"]
            .into_iter()
            .find(|m| !tried.iter().any(|t| t == m))
            .map(str::to_string)
    }

    #[test]
    fn short_names() {
        let short = short_model("google/gemma-4-26b-a4b-it:free");
        assert_eq!(short, "gemma-4-26b-a4b-it");
        let long = short_model("nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free");
        assert_eq!(long, "nemotron-3-nano-omni-30b-a3\u{2026}");
        assert_eq!(short_model("qwen/qwen3.7-flash"), "qwen3.7-flash");
    }

    #[test]
    fn rate_limits_move_to_the_next_free_model_at_most_three_times() {
        let calls = RefCell::new(Vec::new());
        let switched = RefCell::new(Vec::new());
        let out: Result<(String, ()), ApiError> = block(with_failover(
            "a:free",
            pool,
            || true,
            |m| switched.borrow_mut().push(m.to_string()),
            |m| {
                calls.borrow_mut().push(m);
                async { Err(ApiError::http(429, "busy")) }
            },
        ));
        assert_eq!(out.unwrap_err().status, Some(429));
        assert_eq!(*calls.borrow(), ["a:free", "b:free", "c:free"]);
        assert_eq!(*switched.borrow(), ["b:free", "c:free"]);
    }

    #[test]
    fn a_retired_model_is_replaced() {
        let out = block(with_failover(
            "a:free",
            pool,
            || true,
            |_| {},
            |m| async move {
                if m == "a:free" {
                    Err(ApiError::http(404, "gone"))
                } else {
                    Ok(7)
                }
            },
        ));
        assert_eq!(out.unwrap(), ("b:free".to_string(), 7));
    }

    #[test]
    fn auth_paid_and_partial_answers_never_switch() {
        let tries = RefCell::new(0);
        let count = |m: String| {
            *tries.borrow_mut() += 1;
            let err = if m == "a:free" { 401 } else { 429 };
            async move { Err::<(), _>(ApiError::http(err, "x")) }
        };
        let _ = block(with_failover("a:free", pool, || true, |_| {}, count));
        assert_eq!(*tries.borrow(), 1);
        let _ = block(with_failover("paid/model", pool, || true, |_| {}, count));
        assert_eq!(*tries.borrow(), 2);
        let _ = block(with_failover("b:free", pool, || false, |_| {}, count));
        assert_eq!(*tries.borrow(), 3);
        let aborted = block(with_failover(
            "a:free",
            pool,
            || true,
            |_| {},
            |_| async { Err::<(), _>(ApiError::abort()) },
        ));
        assert!(aborted.unwrap_err().is_abort());
    }
}
