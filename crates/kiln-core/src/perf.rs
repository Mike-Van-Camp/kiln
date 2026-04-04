use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::model::Section;

/// A cached disassembled instruction.
#[derive(Debug, Clone)]
pub struct CachedInstruction {
    pub address: u64,
    pub mnemonic: String,
    pub operands: String,
    pub size: usize,
    pub bytes: Vec<u8>,
}

/// LRU cache for instructions, bounded by max_entries.
pub struct InstructionCache {
    entries: HashMap<u64, CachedInstruction>,
    access_order: VecDeque<u64>,
    max_entries: usize,
}

impl InstructionCache {
    /// Create a new cache with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries),
            access_order: VecDeque::with_capacity(max_entries),
            max_entries,
        }
    }

    /// Get an instruction by address, promoting it in the LRU order.
    pub fn get(&mut self, addr: u64) -> Option<&CachedInstruction> {
        if self.entries.contains_key(&addr) {
            // Promote to most-recently-used
            self.access_order.retain(|&a| a != addr);
            self.access_order.push_back(addr);
            self.entries.get(&addr)
        } else {
            None
        }
    }

    /// Insert an instruction into the cache, evicting the oldest if full.
    pub fn insert(&mut self, addr: u64, insn: CachedInstruction) {
        if self.entries.contains_key(&addr) {
            self.access_order.retain(|&a| a != addr);
        } else if self.entries.len() >= self.max_entries {
            self.evict_oldest();
        }
        self.entries.insert(addr, insn);
        self.access_order.push_back(addr);
    }

    /// Number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
    }

    fn evict_oldest(&mut self) {
        if let Some(oldest) = self.access_order.pop_front() {
            self.entries.remove(&oldest);
        }
    }
}

/// Thread-safe progress tracker for long-running operations.
#[derive(Clone)]
pub struct ProgressTracker {
    pub current: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub cancelled: Arc<AtomicBool>,
    pub message: Arc<Mutex<String>>,
}

impl ProgressTracker {
    /// Create a new progress tracker.
    pub fn new() -> Self {
        Self {
            current: Arc::new(AtomicU64::new(0)),
            total: Arc::new(AtomicU64::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
            message: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Set the total number of steps.
    pub fn set_total(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }

    /// Increment the current step by one.
    pub fn increment(&self) {
        self.current.fetch_add(1, Ordering::Relaxed);
    }

    /// Set the progress message.
    pub fn set_message(&self, msg: &str) {
        if let Ok(mut m) = self.message.lock() {
            *m = msg.to_string();
        }
    }

    /// Return progress as a fraction in [0.0, 1.0].
    pub fn progress_fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let current = self.current.load(Ordering::Relaxed);
        (current as f32 / total as f32).min(1.0)
    }

    /// Check whether the operation has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Cancel the operation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Get the current progress message.
    pub fn message(&self) -> String {
        self.message.lock().map(|m| m.clone()).unwrap_or_default()
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Determine which section indices overlap a given address range [start, end).
pub fn sections_in_range(sections: &[Section], start: u64, end: u64) -> Vec<usize> {
    sections
        .iter()
        .enumerate()
        .filter_map(|(i, sec)| {
            let sec_start = sec.address;
            let sec_end = sec.address.saturating_add(sec.size);
            if sec_start < end && sec_end > start {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SectionKind;

    fn make_insn(addr: u64) -> CachedInstruction {
        CachedInstruction {
            address: addr,
            mnemonic: "nop".to_string(),
            operands: String::new(),
            size: 1,
            bytes: vec![0x90],
        }
    }

    fn make_section(address: u64, size: u64) -> Section {
        Section {
            name: "test".to_string(),
            address,
            size,
            file_offset: 0,
            kind: SectionKind::Code,
            readable: true,
            writable: false,
            executable: true,
        }
    }

    #[test]
    fn test_lru_cache_basic() {
        let mut cache = InstructionCache::new(4);
        assert!(cache.is_empty());

        cache.insert(0x1000, make_insn(0x1000));
        cache.insert(0x1001, make_insn(0x1001));
        assert_eq!(cache.len(), 2);

        let insn = cache.get(0x1000).unwrap();
        assert_eq!(insn.address, 0x1000);
        assert_eq!(insn.mnemonic, "nop");

        assert!(cache.get(0x9999).is_none());
    }

    #[test]
    fn test_lru_cache_eviction() {
        let mut cache = InstructionCache::new(3);
        cache.insert(0x1, make_insn(0x1));
        cache.insert(0x2, make_insn(0x2));
        cache.insert(0x3, make_insn(0x3));
        assert_eq!(cache.len(), 3);

        // Inserting a 4th should evict the oldest (0x1)
        cache.insert(0x4, make_insn(0x4));
        assert_eq!(cache.len(), 3);
        assert!(cache.get(0x1).is_none());
        assert!(cache.get(0x2).is_some());
        assert!(cache.get(0x3).is_some());
        assert!(cache.get(0x4).is_some());
    }

    #[test]
    fn test_lru_cache_access_refreshes() {
        let mut cache = InstructionCache::new(3);
        cache.insert(0x1, make_insn(0x1));
        cache.insert(0x2, make_insn(0x2));
        cache.insert(0x3, make_insn(0x3));

        // Access 0x1 to promote it
        cache.get(0x1);

        // Insert 0x4 — should evict 0x2 (now oldest)
        cache.insert(0x4, make_insn(0x4));
        assert!(cache.get(0x2).is_none());
        assert!(cache.get(0x1).is_some());
    }

    #[test]
    fn test_lru_cache_clear() {
        let mut cache = InstructionCache::new(4);
        cache.insert(0x1, make_insn(0x1));
        cache.insert(0x2, make_insn(0x2));
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_progress_tracker_basic() {
        let tracker = ProgressTracker::new();
        tracker.set_total(100);
        tracker.set_message("Working...");
        assert_eq!(tracker.progress_fraction(), 0.0);

        for _ in 0..50 {
            tracker.increment();
        }
        let frac = tracker.progress_fraction();
        assert!((frac - 0.5).abs() < f32::EPSILON);
        assert_eq!(tracker.message(), "Working...");
    }

    #[test]
    fn test_progress_tracker_cancel() {
        let tracker = ProgressTracker::new();
        assert!(!tracker.is_cancelled());
        tracker.cancel();
        assert!(tracker.is_cancelled());
    }

    #[test]
    fn test_progress_tracker_zero_total() {
        let tracker = ProgressTracker::new();
        assert_eq!(tracker.progress_fraction(), 0.0);
    }

    #[test]
    fn test_sections_in_range() {
        let sections = vec![
            make_section(0x1000, 0x100), // 0x1000..0x1100
            make_section(0x2000, 0x200), // 0x2000..0x2200
            make_section(0x3000, 0x100), // 0x3000..0x3100
        ];

        // Range overlapping first section only
        let result = sections_in_range(&sections, 0x1000, 0x1050);
        assert_eq!(result, vec![0]);

        // Range overlapping first and second
        let result = sections_in_range(&sections, 0x1050, 0x2050);
        assert_eq!(result, vec![0, 1]);

        // Range before all sections
        let result = sections_in_range(&sections, 0x0, 0x1000);
        assert!(result.is_empty());

        // Range covering all sections
        let result = sections_in_range(&sections, 0x0, 0x4000);
        assert_eq!(result, vec![0, 1, 2]);
    }
}
