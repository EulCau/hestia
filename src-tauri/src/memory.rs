use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub kind: String,
    pub content: String,
    pub source: String,
    pub confidence: f64,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_used_at: Option<u64>,
    pub pinned: bool,
    pub archived: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryPatch {
    pub kind: Option<String>,
    pub content: Option<String>,
    pub source: Option<String>,
    pub confidence: Option<f64>,
    pub pinned: Option<bool>,
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryDraft {
    pub kind: Option<String>,
    pub content: String,
    pub pinned: Option<bool>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn user_data_root() -> PathBuf {
    if let Ok(dir) = std::env::var("HESTIA_USER_DIR") {
        return PathBuf::from(dir);
    }
    if cfg!(debug_assertions) {
        return project_root().join("usr");
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("hestia");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("hestia");
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("hestia");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("hestia");
    }
    PathBuf::from(".").join("hestia-user-data")
}

fn sanitize_role_id(role_id: &str) -> String {
    let value: String = role_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect();
    if value.trim().is_empty() {
        "default".into()
    } else {
        value
    }
}

fn legacy_memory_path() -> PathBuf {
    user_data_root().join("memory").join("memories.json")
}

fn memory_path(role_id: &str) -> PathBuf {
    user_data_root()
        .join("memory")
        .join(sanitize_role_id(role_id))
        .join("memories.json")
}

fn normalize_kind(kind: &str) -> String {
    match kind.trim() {
        "fact" | "preference" | "project" | "relationship" | "note" => kind.trim().to_string(),
        _ => "note".into(),
    }
}

fn normalize_source(source: &str) -> String {
    match source.trim() {
        "chat" | "user" | "system" => source.trim().to_string(),
        _ => "user".into(),
    }
}

fn clamp_confidence(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

pub fn load_memories(role_id: &str) -> Result<Vec<MemoryItem>, Box<dyn std::error::Error>> {
    let path = memory_path(role_id);
    let path = if path.exists() {
        path
    } else if role_id == "default" && legacy_memory_path().exists() {
        legacy_memory_path()
    } else {
        path
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&content)?)
}

pub fn save_memories(
    role_id: &str,
    memories: &[MemoryItem],
) -> Result<(), Box<dyn std::error::Error>> {
    let path = memory_path(role_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(memories)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn list_memories(
    role_id: &str,
    query: Option<&str>,
    include_archived: bool,
) -> Result<Vec<MemoryItem>, Box<dyn std::error::Error>> {
    let mut memories = load_memories(role_id)?;
    if !include_archived {
        memories.retain(|memory| !memory.archived);
    }
    if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
        let needle = query.to_lowercase();
        memories.retain(|memory| {
            memory.content.to_lowercase().contains(&needle)
                || memory.kind.to_lowercase().contains(&needle)
                || memory.source.to_lowercase().contains(&needle)
        });
    }
    memories.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(memories)
}

pub fn create_memory(
    role_id: &str,
    kind: String,
    content: String,
    source: Option<String>,
    pinned: Option<bool>,
) -> Result<MemoryItem, Box<dyn std::error::Error>> {
    let content = content.trim();
    if content.is_empty() {
        return Err("memory content is empty".into());
    }
    let now = now_ms();
    let item = MemoryItem {
        id: format!("mem_{now}"),
        kind: normalize_kind(&kind),
        content: content.into(),
        source: normalize_source(source.as_deref().unwrap_or("user")),
        confidence: 1.0,
        created_at: now,
        updated_at: now,
        last_used_at: None,
        pinned: pinned.unwrap_or(false),
        archived: false,
    };
    let mut memories = load_memories(role_id)?;
    memories.push(item.clone());
    save_memories(role_id, &memories)?;
    info!(memory_id = %item.id, "memory created");
    Ok(item)
}

pub fn replace_memories_from_drafts(
    role_id: &str,
    drafts: Vec<MemoryDraft>,
    source: &str,
) -> Result<Vec<MemoryItem>, Box<dyn std::error::Error>> {
    let now = now_ms();
    let mut memories = Vec::new();
    for (index, draft) in drafts.into_iter().enumerate() {
        let content = draft.content.trim();
        if content.is_empty() {
            continue;
        }
        memories.push(MemoryItem {
            id: format!("mem_{now}_{index}"),
            kind: normalize_kind(draft.kind.as_deref().unwrap_or("note")),
            content: content.into(),
            source: normalize_source(source),
            confidence: 0.85,
            created_at: now,
            updated_at: now,
            last_used_at: None,
            pinned: draft.pinned.unwrap_or(false),
            archived: false,
        });
    }
    save_memories(role_id, &memories)?;
    Ok(memories)
}

pub fn update_memory(
    role_id: &str,
    id: String,
    patch: MemoryPatch,
) -> Result<MemoryItem, Box<dyn std::error::Error>> {
    let mut memories = load_memories(role_id)?;
    let now = now_ms();
    let index = memories
        .iter()
        .position(|memory| memory.id == id)
        .ok_or_else(|| format!("memory not found: {id}"))?;
    let memory = &mut memories[index];
    if let Some(kind) = patch.kind {
        memory.kind = normalize_kind(&kind);
    }
    if let Some(content) = patch.content {
        let content = content.trim();
        if content.is_empty() {
            return Err("memory content is empty".into());
        }
        memory.content = content.into();
    }
    if let Some(source) = patch.source {
        memory.source = normalize_source(&source);
    }
    if let Some(confidence) = patch.confidence {
        memory.confidence = clamp_confidence(confidence);
    }
    if let Some(pinned) = patch.pinned {
        memory.pinned = pinned;
    }
    if let Some(archived) = patch.archived {
        memory.archived = archived;
    }
    memory.updated_at = now;
    let item = memory.clone();
    save_memories(role_id, &memories)?;
    info!(memory_id = %item.id, "memory updated");
    Ok(item)
}

pub fn delete_memory(role_id: &str, id: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut memories = load_memories(role_id)?;
    let before = memories.len();
    memories.retain(|memory| memory.id != id);
    if memories.len() == before {
        return Err(format!("memory not found: {id}").into());
    }
    save_memories(role_id, &memories)?;
    info!(memory_id = %id, "memory deleted");
    Ok(())
}

fn keyword_score(memory: &MemoryItem, query: &str) -> usize {
    let haystack = format!("{} {}", memory.kind, memory.content).to_lowercase();
    let query_lower = query.to_lowercase();
    let mut score = usize::from(memory.pinned) * 10;
    if !query_lower.trim().is_empty() && haystack.contains(query_lower.trim()) {
        score += 4;
    }
    for token in query_lower
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '-'))
        .filter(|token| token.len() >= 2)
    {
        if haystack.contains(token) {
            score += 1;
        }
    }
    score
}

pub fn relevant_memories(
    role_id: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<MemoryItem>, Box<dyn std::error::Error>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let mut scored: Vec<(usize, MemoryItem)> = load_memories(role_id)?
        .into_iter()
        .filter(|memory| !memory.archived)
        .map(|memory| (keyword_score(&memory, query), memory))
        .filter(|(score, memory)| *score > 0 || memory.pinned)
        .collect();
    scored.sort_by(|(score_a, a), (score_b, b)| {
        score_b
            .cmp(score_a)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    let selected: Vec<MemoryItem> = scored
        .into_iter()
        .take(limit)
        .map(|(_, memory)| memory)
        .collect();
    if !selected.is_empty() {
        mark_memories_used(role_id, selected.iter().map(|memory| memory.id.as_str()))?;
    }
    Ok(selected)
}

fn mark_memories_used<'a>(
    role_id: &str,
    ids: impl Iterator<Item = &'a str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ids: Vec<&str> = ids.collect();
    if ids.is_empty() {
        return Ok(());
    }
    let mut memories = load_memories(role_id)?;
    let now = now_ms();
    for memory in &mut memories {
        if ids.iter().any(|id| *id == memory.id) {
            memory.last_used_at = Some(now);
        }
    }
    save_memories(role_id, &memories)?;
    Ok(())
}

pub fn format_memory_context(memories: &[MemoryItem], language: &str) -> Option<String> {
    if memories.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    for memory in memories {
        lines.push(format!(
            "- [{}{}] {}",
            memory.kind,
            if memory.pinned { ", pinned" } else { "" },
            memory.content
        ));
    }
    Some(crate::personality::render_memory_context_template(
        language,
        &lines.join("\n"),
    ))
}

pub fn memory_storage_path(role_id: &str) -> String {
    memory_path(role_id).display().to_string()
}
