//! Bounded LRU cache for pattern backing pixmaps.

use crate::pattern::Repetition;
use std::collections::HashMap;
use std::sync::Arc;
use tiny_skia::Pixmap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PatternCacheKey {
    pub(crate) pattern_id: u64,
    pub(crate) repetition: Repetition,
    /// Cache dimensions (0,0 sentinel for Repeat mode).
    pub(crate) canvas_width: u32,
    pub(crate) canvas_height: u32,
}

#[derive(Debug)]
struct PatternCacheEntry {
    pixmap: Arc<Pixmap>,
    size_bytes: usize,
    last_used: u64,
}

#[derive(Debug)]
pub(crate) struct PatternPixmapCache {
    max_bytes: usize,
    total_bytes: usize,
    clock: u64,
    entries: HashMap<PatternCacheKey, PatternCacheEntry>,
}

impl PatternPixmapCache {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            total_bytes: 0,
            clock: 0,
            entries: HashMap::new(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.total_bytes = 0;
        self.clock = 0;
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    fn next_tick(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    pub(crate) fn get_or_insert(
        &mut self,
        key: PatternCacheKey,
        create: impl FnOnce() -> Option<Pixmap>,
    ) -> Option<Arc<Pixmap>> {
        if self.entries.contains_key(&key) {
            let tick = self.next_tick();
            let entry = self.entries.get_mut(&key)?;
            entry.last_used = tick;
            return Some(Arc::clone(&entry.pixmap));
        }

        let pixmap = create()?;
        let size_bytes = pixmap.data().len();
        let pixmap = Arc::new(pixmap);

        // Avoid pinning a single oversize pixmap in cache.
        if size_bytes > self.max_bytes {
            return Some(pixmap);
        }

        let tick = self.next_tick();
        self.total_bytes += size_bytes;
        self.entries.insert(
            key,
            PatternCacheEntry {
                pixmap: Arc::clone(&pixmap),
                size_bytes,
                last_used: tick,
            },
        );

        self.evict_to_budget();

        Some(pixmap)
    }

    fn evict_to_budget(&mut self) {
        while self.total_bytes > self.max_bytes {
            let lru_key = self
                .entries
                .iter()
                .min_by_key(|(_key, entry)| entry.last_used)
                .map(|(key, _entry)| *key);

            let Some(key) = lru_key else {
                break;
            };

            if let Some(entry) = self.entries.remove(&key) {
                self.total_bytes = self.total_bytes.saturating_sub(entry.size_bytes);
            } else {
                break;
            }
        }
    }
}
