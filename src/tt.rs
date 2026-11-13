use shakmaty::Move;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    Exact, // the score is the true value
    Lower, // the true value is at least the score (a beta cutoff happened)
    Upper, // the true value is at most the score (no move beat alpha)
}

#[derive(Clone)]
pub struct Entry {
    pub key: u64,
    pub depth: i32,
    pub score: i32,
    pub bound: Bound,
    pub best: Option<Move>,
}

pub struct Tt {
    entries: Vec<Option<Entry>>,
    mask: usize,
}

impl Tt {
    /// 2^20 entries, a few tens of MB. Raise `bits` for a bigger table.
    pub fn new() -> Self {
        Self::with_bits(20)
    }

    pub fn with_bits(bits: u32) -> Self {
        let size = 1usize << bits;
        Tt {
            entries: vec![None; size],
            mask: size - 1,
        }
    }

    pub fn clear(&mut self) {
        for e in self.entries.iter_mut() {
            *e = None;
        }
    }

    pub fn probe(&self, key: u64) -> Option<&Entry> {
        match &self.entries[(key as usize) & self.mask] {
            Some(e) if e.key == key => Some(e),
            _ => None,
        }
    }

    pub fn store(&mut self, key: u64, depth: i32, score: i32, bound: Bound, best: Option<Move>) {
        let slot = &mut self.entries[(key as usize) & self.mask];
        // Don't overwrite a deeper result for the same position.
        if let Some(old) = slot.as_ref()
            && old.key == key
            && old.depth > depth
        {
            return;
        }
        *slot = Some(Entry {
            key,
            depth,
            score,
            bound,
            best,
        });
    }
}
