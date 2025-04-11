#[derive(Debug, Clone)]
pub struct Bitset {
    bits: Vec<u64>,
}

impl Bitset {
    pub fn new() -> Self {
        Self { bits: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let len = (capacity + 63) / 64;
        Self { bits: vec![0; len] }
    }

    pub fn reserve(&mut self, additional: usize) {
        let len = (self.bits.len() * 64 + additional + 63) / 64;
        if len > self.bits.len() {
            self.bits.resize(len, 0);
        }
    }

    pub fn set(&mut self, index: usize) {
        let (word, bit) = (index / 64, index % 64);
        if word >= self.bits.len() {
            self.bits.resize(word + 1, 0);
        }
        self.bits[word] |= 1 << bit;
    }

    pub fn reset_bit(&mut self, index: usize) {
        let (word, bit) = (index / 64, index % 64);
        if word < self.bits.len() {
            self.bits[word] &= !(1 << bit);
        }
    }

    pub fn get(&self, index: usize) -> bool {
        let (word, bit) = (index / 64, index % 64);
        word < self.bits.len() && (self.bits[word] & (1 << bit)) != 0
    }

    /// Checks if all the bits in `other` are set in `self`.
    /// Returns `true` if `self` contains all bits of `other`, otherwise `false`.
    pub fn contains(&self, other: &Self) -> bool {
        if other.bits.len() > self.bits.len() {
            return false;
        }

        self.bits
            .iter()
            .zip(other.bits.iter())
            .all(|(a, b)| a & b == *b)
    }

    /// Checks if any of the bits in `other` are set in `self`.
    /// Returns `true` if there is an intersection, otherwise `false`.
    pub fn intersects(&self, other: &Self) -> bool {
        if other.bits.len() > self.bits.len() {
            return false;
        }

        self.bits
            .iter()
            .zip(other.bits.iter())
            .any(|(a, b)| a & b != 0)
    }

    pub fn len(&self) -> usize {
        self.bits.len() * 64
    }

    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    pub fn reset(&mut self) {
        for word in &mut self.bits {
            *word = 0;
        }
    }

    pub fn clear(&mut self) {
        self.bits.clear();
    }

    pub fn iter(&self) -> BitsetIter {
        BitsetIter {
            bits: self,
            word: 0,
            bit: 0,
        }
    }
}

impl Default for Bitset {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BitsetIter<'a> {
    bits: &'a Bitset,
    word: usize,
    bit: usize,
}

impl<'a> Iterator for BitsetIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while self.word < self.bits.bits.len() {
            let word = self.bits.bits[self.word];
            while self.bit < 64 {
                if (word & (1 << self.bit)) != 0 {
                    let index = self.word * 64 + self.bit;
                    self.bit += 1;
                    return Some(index);
                }
                self.bit += 1;
            }
            self.word += 1;
            self.bit = 0;
        }
        None
    }
}

pub struct AccessBitset {
    bits: Bitset,
}

impl AccessBitset {
    pub fn new() -> Self {
        Self {
            bits: Bitset::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bits: Bitset::with_capacity(capacity * 2),
        }
    }

    pub fn get(&self, index: usize) -> (bool, bool) {
        let index = index * 2;
        let read = self.bits.get(index);
        let write = self.bits.get(index + 1);
        (read, write)
    }

    /// Sets the read bit for the given index.
    /// Returns `true` if the read bit was successfully set, otherwise `false`.
    pub fn read(&mut self, index: usize) -> bool {
        if self.bits.get(index + 1) {
            return false;
        } else {
            self.bits.set(index);
            return true;
        }
    }

    /// Sets the write bit for the given index.
    /// Returns `true` if the write bit was successfully set, otherwise `false`.
    pub fn write(&mut self, index: usize) -> bool {
        let (read, write) = self.get(index);
        if read || write {
            return false;
        } else {
            self.bits.set(index + 1);
            return true;
        }
    }

    pub fn reset_access(&mut self, index: usize) {
        let index = index * 2;
        self.bits.reset_bit(index);
        self.bits.reset_bit(index + 1);
    }
}
