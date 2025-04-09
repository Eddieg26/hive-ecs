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

    pub fn clear(&mut self, index: usize) {
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

    pub fn clear_all(&mut self) {
        for word in &mut self.bits {
            *word = 0;
        }
    }

    pub fn reset(&mut self) {
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
    read: Bitset,
    write: Bitset,
}

impl AccessBitset {
    pub fn new() -> Self {
        Self {
            read: Bitset::new(),
            write: Bitset::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            read: Bitset::with_capacity(capacity),
            write: Bitset::with_capacity(capacity),
        }
    }

    /// Sets the read bit for the given index.
    /// Returns `true` if the read bit was successfully set, otherwise `false`.
    pub fn read(&mut self, index: usize) -> bool {
        if self.write.get(index) {
            false
        } else {
            self.read.set(index);
            true
        }
    }

    /// Sets the write bit for the given index.
    /// Returns `true` if the write bit was successfully set, otherwise `false`.
    pub fn write(&mut self, index: usize) -> bool {
        if self.read.get(index) || self.write.get(index) {
            false
        } else {
            self.write.set(index);
            true
        }
    }

    pub fn clear_read(&mut self, index: usize) {
        self.read.clear(index);
    }

    pub fn clear_write(&mut self, index: usize) {
        self.write.clear(index);
    }

    pub fn get_read(&self, index: usize) -> bool {
        self.read.get(index)
    }

    pub fn get_write(&self, index: usize) -> bool {
        self.write.get(index)
    }
}
