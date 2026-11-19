//! Play queue of unified track ids (addendum §1: queue items are tracks, not sources, so the
//! queue keeps working when one service is unavailable).

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

/// Small xorshift PRNG: shuffle needs no crypto, and tests need a seed.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn seeded(seed: u64) -> Self {
        Self(seed.max(1))
    }

    /// Seeded from the OS RNG (falls back to the clock).
    pub fn from_entropy() -> Self {
        let mut bytes = [0u8; 8];
        let seed = if getrandom::fill(&mut bytes).is_ok() {
            u64::from_le_bytes(bytes)
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E37_79B9_7F4A_7C15)
        };
        Self::seeded(seed)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        usize::try_from(self.next() % (n as u64)).unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct Queue {
    items: Vec<String>,
    /// Play order as indices into `items` (identity unless shuffled).
    order: Vec<usize>,
    /// Position in `order`.
    pos: Option<usize>,
    shuffle: bool,
    repeat: RepeatMode,
    rng: Rng,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new(Rng::from_entropy())
    }
}

impl Queue {
    pub fn new(rng: Rng) -> Self {
        Self { items: Vec::new(), order: Vec::new(), pos: None, shuffle: false, repeat: RepeatMode::Off, rng }
    }

    /// Replaces the queue and starts at `start` (clamped).
    pub fn set(&mut self, items: Vec<String>, start: usize) {
        self.items = items;
        if self.items.is_empty() {
            self.order.clear();
            self.pos = None;
            return;
        }
        let start = start.min(self.items.len() - 1);
        self.order = (0..self.items.len()).collect();
        self.pos = Some(start);
        if self.shuffle {
            self.reshuffle_keeping(start);
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn current(&self) -> Option<&str> {
        let idx = *self.order.get(self.pos?)?;
        self.items.get(idx).map(String::as_str)
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    /// Turning shuffle on keeps the current track first and randomises the rest; turning it
    /// off restores list order and keeps the current track.
    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        let Some(current) = self.pos.and_then(|p| self.order.get(p).copied()) else {
            self.order = (0..self.items.len()).collect();
            return;
        };
        if on {
            self.reshuffle_keeping(current);
        } else {
            self.order = (0..self.items.len()).collect();
            self.pos = Some(current);
        }
    }

    /// Order = [current, random permutation of the rest]; pos = 0.
    fn reshuffle_keeping(&mut self, current: usize) {
        let mut rest: Vec<usize> = (0..self.items.len()).filter(|&i| i != current).collect();
        // Fisher–Yates.
        for i in (1..rest.len()).rev() {
            let j = self.rng.below(i + 1);
            rest.swap(i, j);
        }
        self.order = std::iter::once(current).chain(rest).collect();
        self.pos = Some(0);
    }

    /// Advances. `manual` = the user pressed Next (repeat-one still moves on); automatic
    /// advance at the end of a track repeats under `One`. Returns the new current track or
    /// `None` at the end of the queue (repeat off).
    pub fn next(&mut self, manual: bool) -> Option<&str> {
        let pos = self.pos?;
        if !manual && self.repeat == RepeatMode::One {
            return self.current();
        }
        if pos + 1 < self.order.len() {
            self.pos = Some(pos + 1);
        } else if self.repeat == RepeatMode::All || (manual && self.repeat == RepeatMode::One) {
            if self.shuffle {
                // New cycle, new order; avoid playing the same track twice in a row.
                let last = self.order[pos];
                self.reshuffle_keeping(last);
                self.pos = Some(if self.order.len() > 1 { 1 } else { 0 });
            } else {
                self.pos = Some(0);
            }
        } else {
            return None;
        }
        self.current()
    }

    /// Goes back one item (wraps under repeat-all). Restarting the current track when it has
    /// played for a while is the player's decision, not the queue's.
    pub fn prev(&mut self) -> Option<&str> {
        let pos = self.pos?;
        if pos > 0 {
            self.pos = Some(pos - 1);
        } else if self.repeat == RepeatMode::All && !self.order.is_empty() {
            self.pos = Some(self.order.len() - 1);
        }
        self.current()
    }

    /// Inserts a track right after the current one.
    pub fn play_next(&mut self, track_id: String) {
        self.items.push(track_id);
        let idx = self.items.len() - 1;
        match self.pos {
            Some(p) => self.order.insert(p + 1, idx),
            None => {
                self.order.push(idx);
                self.pos = Some(0);
            }
        }
    }

    pub fn append(&mut self, track_id: String) {
        self.items.push(track_id);
        self.order.push(self.items.len() - 1);
        if self.pos.is_none() {
            self.pos = Some(0);
        }
    }

    /// Upcoming tracks in play order (after the current one).
    pub fn upcoming(&self) -> Vec<&str> {
        let start = self.pos.map_or(0, |p| p + 1);
        self.order[start.min(self.order.len())..]
            .iter()
            .filter_map(|&i| self.items.get(i).map(String::as_str))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: usize) -> Queue {
        let mut q = Queue::new(Rng::seeded(7));
        q.set((0..n).map(|i| format!("t{i}")).collect(), 0);
        q
    }

    #[test]
    fn next_and_prev_in_order() {
        let mut q = q(3);
        assert_eq!(q.current(), Some("t0"));
        assert_eq!(q.next(true), Some("t1"));
        assert_eq!(q.next(false), Some("t2"));
        assert_eq!(q.next(false), None, "end of queue with repeat off");
        assert_eq!(q.prev(), Some("t1"));
        assert_eq!(q.prev(), Some("t0"));
        assert_eq!(q.prev(), Some("t0"), "stays at the start");
    }

    #[test]
    fn repeat_all_wraps_both_ways() {
        let mut q = q(2);
        q.set_repeat(RepeatMode::All);
        q.next(false);
        assert_eq!(q.next(false), Some("t0"));
        assert_eq!(q.prev(), Some("t1"));
    }

    #[test]
    fn repeat_one_repeats_on_auto_but_not_on_manual_next() {
        let mut q = q(3);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.next(false), Some("t0"));
        assert_eq!(q.next(true), Some("t1"));
    }

    #[test]
    fn shuffle_keeps_current_first_and_plays_each_once() {
        let mut q = q(20);
        q.next(true);
        q.next(true); // current = t2
        q.set_shuffle(true);
        assert_eq!(q.current(), Some("t2"));
        let mut seen = vec![q.current().unwrap().to_owned()];
        while let Some(t) = q.next(false) {
            seen.push(t.to_owned());
        }
        let mut sorted = seen.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 20, "every track exactly once");
        assert_ne!(seen, (0..20).map(|i| format!("t{i}")).collect::<Vec<_>>(), "order changed");
        let current = q.current().unwrap().to_owned();
        q.set_shuffle(false);
        assert_eq!(q.current(), Some(current.as_str()), "turning shuffle off keeps the current track");
    }

    #[test]
    fn shuffle_repeat_all_starts_a_new_cycle_without_immediate_repeat() {
        let mut q = q(5);
        q.set_shuffle(true);
        q.set_repeat(RepeatMode::All);
        let mut last = q.current().unwrap().to_owned();
        for _ in 0..25 {
            let next = q.next(false).unwrap().to_owned();
            assert_ne!(next, last);
            last = next;
        }
    }

    #[test]
    fn play_next_and_append() {
        let mut q = q(2);
        q.append("z".into());
        q.play_next("n".into());
        assert_eq!(q.upcoming(), vec!["n", "t1", "z"]);
        let mut empty = Queue::new(Rng::seeded(1));
        assert_eq!(empty.next(true), None);
        empty.append("a".into());
        assert_eq!(empty.current(), Some("a"));
    }

    #[test]
    fn start_index_is_clamped() {
        let mut q = Queue::new(Rng::seeded(1));
        q.set(vec!["a".into(), "b".into()], 9);
        assert_eq!(q.current(), Some("b"));
        q.set(vec![], 0);
        assert!(q.is_empty() && q.current().is_none());
    }
}
