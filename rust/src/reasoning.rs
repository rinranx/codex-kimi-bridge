use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const DEFAULT_TTL: Duration = Duration::from_secs(2 * 60 * 60);
const DEFAULT_MAX_ENTRIES: usize = 512;
const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug)]
struct Entry {
    value: String,
    bytes: usize,
    expires_at: Instant,
}

#[derive(Debug, Default)]
struct Inner {
    entries: HashMap<String, Entry>,
    order: VecDeque<String>,
    total_bytes: usize,
}

#[derive(Debug)]
pub struct ReasoningStore {
    ttl: Duration,
    max_entries: usize,
    max_bytes: usize,
    inner: Mutex<Inner>,
}

impl Default for ReasoningStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReasoningStore {
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_TTL, DEFAULT_MAX_ENTRIES, DEFAULT_MAX_BYTES)
    }

    pub fn with_limits(ttl: Duration, max_entries: usize, max_bytes: usize) -> Self {
        Self {
            ttl: if ttl.is_zero() { DEFAULT_TTL } else { ttl },
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
            inner: Mutex::new(Inner::default()),
        }
    }

    pub fn set(&self, call_id: &str, reasoning_content: &str) -> bool {
        if call_id.is_empty() || reasoning_content.is_empty() {
            return false;
        }
        let bytes = reasoning_content.len();
        if bytes > self.max_bytes {
            return false;
        }

        let mut inner = self.inner.lock().expect("reasoning store mutex poisoned");
        remove_locked(&mut inner, call_id);
        inner.entries.insert(
            call_id.to_owned(),
            Entry {
                value: reasoning_content.to_owned(),
                bytes,
                expires_at: Instant::now() + self.ttl,
            },
        );
        inner.order.push_back(call_id.to_owned());
        inner.total_bytes += bytes;
        self.prune_locked(&mut inner);
        true
    }

    pub fn get(&self, call_id: &str) -> Option<String> {
        let mut inner = self.inner.lock().expect("reasoning store mutex poisoned");
        let expired = inner
            .entries
            .get(call_id)
            .is_some_and(|entry| entry.expires_at <= Instant::now());
        if expired {
            remove_locked(&mut inner, call_id);
            return None;
        }
        let value = inner.entries.get(call_id)?.value.clone();
        inner.order.retain(|existing| existing != call_id);
        inner.order.push_back(call_id.to_owned());
        Some(value)
    }

    pub fn clear(&self) {
        *self.inner.lock().expect("reasoning store mutex poisoned") = Inner::default();
    }

    fn prune_locked(&self, inner: &mut Inner) {
        let now = Instant::now();
        let expired: Vec<String> = inner
            .entries
            .iter()
            .filter(|(_, entry)| entry.expires_at <= now)
            .map(|(key, _)| key.clone())
            .collect();
        for key in expired {
            remove_locked(inner, &key);
        }
        while inner.entries.len() > self.max_entries || inner.total_bytes > self.max_bytes {
            let Some(oldest) = inner.order.pop_front() else {
                break;
            };
            if let Some(entry) = inner.entries.remove(&oldest) {
                inner.total_bytes = inner.total_bytes.saturating_sub(entry.bytes);
            }
        }
    }
}

fn remove_locked(inner: &mut Inner, call_id: &str) {
    if let Some(entry) = inner.entries.remove(call_id) {
        inner.total_bytes = inner.total_bytes.saturating_sub(entry.bytes);
    }
    inner.order.retain(|existing| existing != call_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_is_bounded_and_rejects_invalid_entries() {
        let store = ReasoningStore::with_limits(Duration::from_secs(60), 2, 32);
        assert!(!store.set("", "secret"));
        assert!(!store.set("call_oversize", &"x".repeat(33)));
        assert!(store.set("call_1", "one"));
        assert!(store.set("call_2", "two"));
        assert!(store.set("call_3", "three"));
        assert_eq!(store.get("call_1"), None);
        assert_eq!(store.get("call_2").as_deref(), Some("two"));
        assert_eq!(store.get("call_3").as_deref(), Some("three"));
    }
}
