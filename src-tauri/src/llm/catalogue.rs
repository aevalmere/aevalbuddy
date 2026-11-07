//! The curated OpenRouter model table, the live catalogue and the rules that
//! keep a stored model id alive (PARITY MODEL-02..07, FAIL-05). Port of the
//! catalogue half of lib/openrouter.js; pure functions over the `/models`
//! data array so the cache and the network live in client.rs.

use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Free,
    Paid,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Free => "free",
            Tier::Paid => "paid",
        }
    }
}

/// What a model is for. Vision answers from the screen; text is the brain,
/// which never sees an image; find and agent are their own pickers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Vision,
    Text,
    Find,
    Agent,
}

impl Kind {
    /// `models:list` and `listModels`: text, find and agent by name, anything
    /// else is vision.
    pub fn from_list_kind(k: Option<&str>) -> Kind {
        match k {
            Some("text") => Kind::Text,
            Some("find") => Kind::Find,
            Some("agent") => Kind::Agent,
            _ => Kind::Vision,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Vision => "vision",
            Kind::Text => "text",
            Kind::Find => "find",
            Kind::Agent => "agent",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Curated {
    pub id: &'static str,
    pub tier: Tier,
    pub note: &'static str,
}

const fn free(id: &'static str, note: &'static str) -> Curated {
    Curated {
        id,
        tier: Tier::Free,
        note,
    }
}

const fn paid(id: &'static str, note: &'static str) -> Curated {
    Curated {
        id,
        tier: Tier::Paid,
        note,
    }
}

// Reading accuracy is the whole job, whether this model answers or only reads
// for the brain. Re-verified 2026-11-07 against
// https://openrouter.ai/api/v1/models: every id below is listed, and every
// CURATED entry takes image input.
pub const DEFAULT_MODEL: &str = "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free";
pub const DEFAULT_BRAIN: &str = "nvidia/nemotron-3.5-super-vl:free";
pub const DEFAULT_FIND: &str = "nvidia/nemotron-3.5-lightning:free";
pub const DEFAULT_AGENT: &str = "nvidia/nemotron-3.5-super-vl:free";

#[rustfmt::skip]
pub const CURATED: [Curated; 10] = [
    free(
        "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free",
        "Built as a perception sub-agent, which is the job here: read the screen accurately, \
because everything else is built on that reading. 3B active parameters, so it is not slow. The \
default.",
    ),
    free(
        "google/gemma-4-26b-a4b-it:free",
        "Only 4B parameters active per token, so it is the quickest of the free ones. Pick it \
for speed over accuracy.",
    ),
    free(
        "dots-studio/dots-3-note-preview:free",
        "Half a million tokens of context. Handles very long conversations.",
    ),
    free(
        "google/gemma-4-31b-it:free",
        "The larger Gemma. Better answers, slower to start.",
    ),
    free(
        "nvidia/nemotron-3.5-super-vl:free",
        "NVIDIA, 120B with 12B active, built for GUI screenshots and documents. The most careful \
free reader, and slower to start than the default.",
    ),
    paid(
        "qwen/qwen3.7-flash",
        "Quick to start talking and reliable at reading a screen. The one to pick if you add \
credit.",
    ),
    paid(
        "deepseek/deepseek-v4-flash-vision-exp",
        "DeepSeek, built for vision. Good on dense text and code.",
    ),
    paid(
        "z-ai/glm-4.6v",
        "GLM. Strong at reading interfaces and dialogs.",
    ),
    paid(
        "bytedance-seed/seed-1.6-flash",
        "Cheapest of the quick ones. Short answers.",
    ),
    paid(
        "z-ai/glm-5v-turbo",
        "The most capable vision model here. Slower to start, better at multi-step problems.",
    ),
];

#[rustfmt::skip]
pub const BRAINS: [Curated; 5] = [
    free(
        "nvidia/nemotron-3.5-super-vl:free",
        "Most of Ultra at a fraction of the wait, and it can call tools. The default.",
    ),
    free(
        "nvidia/nemotron-3-ultra-550b-a55b:free",
        "550B parameters, a million tokens of context. Heavy, and slow to start.",
    ),
    free(
        "nvidia/nemotron-3.5-lightning:free",
        "Built for speed. Use it when the reading is already doing the hard part.",
    ),
    paid(
        "z-ai/glm-5.3",
        "GLM 5.3. The sharpest brain here, for a fraction of a cent.",
    ),
    paid(
        "deepseek/deepseek-v4-flash",
        "DeepSeek Flash. Answers almost instantly for a fraction of a cent.",
    ),
];

pub fn tier_label(tier: Tier) -> &'static str {
    match tier {
        Tier::Free => "Free",
        Tier::Paid => "Paid, a fraction of a cent per question",
    }
}

/// The curated pool a kind picks from: vision is CURATED, agent is brains
/// then curated (deduplicated), text and find are the brains.
pub fn pool_for(kind: Kind) -> Vec<&'static Curated> {
    match kind {
        Kind::Vision => CURATED.iter().collect(),
        Kind::Agent => {
            let mut out: Vec<&'static Curated> = Vec::new();
            for c in BRAINS.iter().chain(CURATED.iter()) {
                if !out.iter().any(|o| o.id == c.id) {
                    out.push(c);
                }
            }
            out
        }
        Kind::Text | Kind::Find => BRAINS.iter().collect(),
    }
}

pub fn default_for(kind: Kind) -> &'static str {
    match kind {
        Kind::Vision => DEFAULT_MODEL,
        Kind::Text => DEFAULT_BRAIN,
        Kind::Find => DEFAULT_FIND,
        Kind::Agent => DEFAULT_AGENT,
    }
}

/// OpenRouter has shipped two shapes for model modality over time; accept
/// either.
pub fn accepts_images(m: &Value) -> bool {
    let a = &m["architecture"];
    if !a.is_object() {
        return false;
    }
    if let Some(modality) = a["modality"].as_str() {
        if modality.contains("image") {
            return true;
        }
    }
    a["input_modalities"]
        .as_array()
        .is_some_and(|list| list.iter().any(|x| x.as_str() == Some("image")))
}

pub fn lists_tools(m: &Value) -> bool {
    m["supported_parameters"]
        .as_array()
        .is_some_and(|list| list.iter().any(|x| x.as_str() == Some("tools")))
}

fn find<'a>(all: &'a [Value], id: &str) -> Option<&'a Value> {
    all.iter().find(|m| m["id"].as_str() == Some(id))
}

/// True when the catalogue lists native tool calling for this model.
pub fn supports_tools(all: Option<&[Value]>, id: &str) -> bool {
    all.and_then(|a| find(a, id)).is_some_and(lists_tools)
}

/// True when the catalogue says this model takes images.
pub fn sees_images(all: Option<&[Value]>, id: &str) -> bool {
    all.and_then(|a| find(a, id)).is_some_and(accepts_images)
}

/// JS `Number(x)`: numbers, numeric strings ("" and null are 0), anything
/// else NaN.
pub fn js_number(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) if s.trim().is_empty() => 0.0,
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        Value::Null => 0.0,
        Value::Bool(b) => f64::from(u8::from(*b)),
        _ => f64::NAN,
    }
}

/// `Number(obj && obj.field)` where a missing field is `undefined` (NaN).
fn js_field_number(obj: &Value, key: &str) -> f64 {
    match obj.get(key) {
        Some(v) => js_number(v),
        None => f64::NAN,
    }
}

/// JS `Math.round` for the non-negative values used here.
fn js_round(n: f64) -> f64 {
    (n + 0.5).floor()
}

pub fn format_context(n: f64) -> String {
    if !n.is_finite() || n == 0.0 {
        return String::new();
    }
    if n >= 1_000_000.0 {
        return format!("{}M context", js_round(n / 1_000_000.0));
    }
    format!("{}K context", js_round(n / 1024.0))
}

/// Price per million tokens, the unit OpenRouter publishes.
pub fn format_price(pricing: &Value) -> String {
    let inp = js_field_number(pricing, "prompt");
    let out = js_field_number(pricing, "completion");
    if !inp.is_finite() || !out.is_finite() {
        return String::new();
    }
    if inp == 0.0 && out == 0.0 {
        return "Free".to_string();
    }
    format!("${:.2} in / ${:.2} out per million tokens", inp * 1e6, out * 1e6)
}

pub fn describe(m: &Value) -> String {
    let parts = [
        format_price(&m["pricing"]),
        format_context(js_field_number(m, "context_length")),
    ];
    parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn entry(c: &Curated) -> Value {
    json!({ "id": c.id, "name": c.id, "note": c.note, "tier": c.tier.as_str(), "detail": "" })
}

/// Shown when the live list cannot be fetched, so the picker is never empty.
pub fn fallback_from(list: &[&Curated], include_paid: bool) -> Vec<Value> {
    list.iter()
        .filter(|c| include_paid || c.tier == Tier::Free)
        .map(|c| entry(c))
        .collect()
}

pub fn group_by(models: &[Value]) -> Vec<Value> {
    [Tier::Free, Tier::Paid]
        .into_iter()
        .map(|tier| {
            let list: Vec<Value> = models
                .iter()
                .filter(|m| m["tier"].as_str() == Some(tier.as_str()))
                .cloned()
                .collect();
            json!({ "tier": tier.as_str(), "label": tier_label(tier), "models": list })
        })
        .filter(|g| g["models"].as_array().is_some_and(|m| !m.is_empty()))
        .collect()
}

fn offline_list(curated: &[&Curated], include_paid: bool) -> Value {
    let groups = group_by(&fallback_from(curated, include_paid));
    json!({ "groups": groups, "live": false })
}

/// The model catalogue for a settings picker: `{groups, live}`. Paid models
/// stay hidden until asked for; a retired or image-less (vision) model is
/// dropped rather than offered and failed at query time.
pub fn list_models(kind: Kind, include_paid: bool, all: Option<&[Value]>) -> Value {
    let curated = pool_for(kind);
    let Some(all) = all.filter(|a| !a.is_empty()) else {
        return offline_list(&curated, include_paid);
    };
    let mut models = Vec::new();
    for c in &curated {
        if !include_paid && c.tier != Tier::Free {
            continue;
        }
        let Some(live) = find(all, c.id) else {
            continue;
        };
        if kind == Kind::Vision && !accepts_images(live) {
            continue;
        }
        let name = match live["name"].as_str() {
            Some(n) if !n.is_empty() => n,
            _ => c.id,
        };
        let mut m = json!({
            "id": c.id,
            "name": name,
            "note": c.note,
            "tier": c.tier.as_str(),
            "detail": describe(live),
        });
        if kind == Kind::Agent {
            // A model without native tools still works, through the
            // one-JSON-object-per-turn fallback.
            let tools = lists_tools(live);
            m["tag"] = json!(if tools { "Tools" } else { "JSON mode" });
            m["tools"] = json!(tools);
        }
        models.push(m);
    }
    if models.is_empty() {
        return offline_list(&curated, include_paid);
    }
    json!({ "groups": group_by(&models), "live": true })
}

/// The model to actually use for a stored or default id, given a catalogue.
/// A retired id is swapped for the kind's default, or the first curated free
/// model still live; never moved from one paid model to another. Without a
/// catalogue nothing is known to be gone, so the id is kept.
pub fn pick_live(kind: Kind, id: &str, all: Option<&[Value]>) -> String {
    let Some(all) = all.filter(|a| !a.is_empty()) else {
        return id.to_string();
    };
    // Only the screen model has to take images. Find answers in text and the
    // agent falls back to JSON turns, so for those any listed model will do.
    let usable = |mid: &str| usable_for(kind, all, mid);
    if usable(id) {
        return id.to_string();
    }
    let fallback = default_for(kind);
    if usable(fallback) {
        return fallback.to_string();
    }
    pool_for(kind)
        .into_iter()
        .find(|c| c.tier == Tier::Free && usable(c.id))
        .map_or_else(|| id.to_string(), |c| c.id.to_string())
}

fn usable_for(kind: Kind, all: &[Value], id: &str) -> bool {
    find(all, id).is_some_and(|m| kind != Kind::Vision || accepts_images(m))
}

/// The next free model to try after one has failed: the first untried free
/// one in the kind's pool (find uses the brains). An agent keeps its tool
/// dialect, so only tools-capable stand-ins once the catalogue is known.
pub fn next_free_model(kind: Kind, tried: &[String], all: Option<&[Value]>) -> Option<String> {
    let pool = pool_for(if kind == Kind::Find { Kind::Text } else { kind });
    let ok = |id: &str| kind != Kind::Agent || all.is_none() || supports_tools(all, id);
    pool.into_iter()
        .find(|m| m.tier == Tier::Free && !tried.iter().any(|t| t == m.id) && ok(m.id))
        .map(|m| m.id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOGUE: &str = include_str!("../../../tools/fixtures/llm/catalogue.json");

    fn catalogue() -> Vec<Value> {
        let v: Value = serde_json::from_str(CATALOGUE).unwrap();
        v["data"].as_array().unwrap().clone()
    }

    fn ids(list: &[&Curated]) -> Vec<&'static str> {
        list.iter().map(|c| c.id).collect()
    }

    #[test]
    fn curated_table_order_and_tiers() {
        assert_eq!(CURATED.len(), 10);
        assert_eq!(BRAINS.len(), 5);
        let free_vision = CURATED.iter().filter(|c| c.tier == Tier::Free).count();
        assert_eq!(free_vision, 5);
        assert_eq!(CURATED[0].id, DEFAULT_MODEL);
        assert_eq!(BRAINS[0].id, DEFAULT_BRAIN);
        let agent = pool_for(Kind::Agent);
        assert_eq!(agent.len(), 14);
        assert_eq!(ids(&agent[..3]), ids(&pool_for(Kind::Text)[..3]));
        assert_eq!(ids(&pool_for(Kind::Find)), ids(&pool_for(Kind::Text)));
    }

    #[test]
    fn defaults_are_live_in_the_recorded_catalogue() {
        // Recorded from https://openrouter.ai/api/v1/models on 2026-11-07.
        let all = catalogue();
        for id in [DEFAULT_MODEL, DEFAULT_BRAIN, DEFAULT_FIND, DEFAULT_AGENT] {
            assert!(find(&all, id).is_some(), "{id} missing");
        }
        assert!(accepts_images(find(&all, DEFAULT_MODEL).unwrap()));
        for c in CURATED.iter().chain(BRAINS.iter()) {
            assert!(find(&all, c.id).is_some(), "{} missing", c.id);
        }
    }

    #[test]
    fn detail_lines() {
        let m = json!({"pricing": {"prompt": "0", "completion": "0"}, "context_length": 262144});
        assert_eq!(describe(&m), "Free · 256K context");
        let m = json!({"pricing": {"prompt": "0.0000002", "completion": "0.0000015"},
            "context_length": 1048576});
        assert_eq!(describe(&m), "$0.20 in / $1.50 out per million tokens · 1M context");
        assert_eq!(describe(&json!({})), "");
    }

    #[test]
    fn list_hides_paid_and_tags_agents() {
        let all = catalogue();
        let v = list_models(Kind::Vision, false, Some(&all));
        assert_eq!(v["live"], json!(true));
        let groups = v["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["label"], json!("Free"));
        let with_paid = list_models(Kind::Vision, true, Some(&all));
        let g = with_paid["groups"].as_array().unwrap();
        assert_eq!(g[1]["label"], json!("Paid, a fraction of a cent per question"));
        let agent = list_models(Kind::Agent, false, Some(&all));
        let first = &agent["groups"][0]["models"][0];
        assert_eq!(first["tag"], json!("Tools"));
        assert_eq!(first["tools"], json!(true));
        let off = list_models(Kind::Vision, false, None);
        assert_eq!(off["live"], json!(false));
        assert_eq!(off["groups"][0]["models"][0]["detail"], json!(""));
    }

    #[test]
    fn retired_ids_heal_to_a_live_free_model() {
        let all = catalogue();
        let a = Some(all.as_slice());
        assert_eq!(pick_live(Kind::Vision, "gone/model:free", a), DEFAULT_MODEL);
        assert_eq!(pick_live(Kind::Vision, "gone/model:free", None), "gone/model:free");
        assert_eq!(pick_live(Kind::Text, DEFAULT_FIND, a), DEFAULT_FIND);
        // A text model is not a usable screen model.
        assert_eq!(pick_live(Kind::Vision, DEFAULT_FIND, a), DEFAULT_MODEL);
        let without_default: Vec<Value> = all
            .iter()
            .filter(|m| m["id"].as_str() != Some(DEFAULT_MODEL))
            .cloned()
            .collect();
        let healed = pick_live(Kind::Vision, "gone", Some(&without_default));
        assert_eq!(healed, "google/gemma-4-26b-a4b-it:free");
    }

    #[test]
    fn failover_walks_the_free_pool() {
        let tried = vec![DEFAULT_MODEL.to_string()];
        let next = next_free_model(Kind::Vision, &tried, None);
        assert_eq!(next.as_deref(), Some("google/gemma-4-26b-a4b-it:free"));
        let tried = vec![DEFAULT_FIND.to_string(), DEFAULT_BRAIN.to_string()];
        let next = next_free_model(Kind::Find, &tried, None);
        assert_eq!(next.as_deref(), Some("nvidia/nemotron-3-ultra-550b-a55b:free"));
        let all: Vec<String> = pool_for(Kind::Text).iter().map(|c| c.id.to_string()).collect();
        assert_eq!(next_free_model(Kind::Text, &all, None), None);
        let cat = vec![json!({"id": DEFAULT_BRAIN, "supported_parameters": []})];
        let next = next_free_model(Kind::Agent, &[], Some(&cat));
        assert_eq!(next, None);
    }
}
