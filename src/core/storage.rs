use std::{
    hash::Hash,
    ops::{Index, IndexMut},
};

pub trait SparseIndex: Copy + Clone + PartialEq + Eq + Hash {
    fn to_usize(self) -> usize;
    fn from_usize(index: usize) -> Self;
}

macro_rules! impl_sparse_index {
    ($($ty:ty),+) => {
        $(impl SparseIndex for $ty {
            #[inline]
            fn to_usize(self) -> usize {
                self as usize
            }

            #[inline]
            fn from_usize(value: usize) -> Self {
                value as $ty
            }
        })*
    };
}

impl_sparse_index!(u8, u16, u32, u64, usize);

pub struct SparseArray<I, V = I> {
    values: Vec<Option<V>>,
    _marker: std::marker::PhantomData<I>,
}

impl<I, V> SparseArray<I, V> {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }
}

impl<I, V> Default for SparseArray<I, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, V> SparseArray<I, V>
where
    I: SparseIndex,
{
    pub fn insert(&mut self, index: I, value: V) {
        let index = index.to_usize();
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        self.values[index] = Some(value);
    }

    pub fn reserve(&mut self, index: I) {
        if index.to_usize() > self.values.len() {
            self.values.resize_with(index.to_usize(), || None);
        }
    }

    pub fn get(&self, index: I) -> Option<&V> {
        let index = index.to_usize();
        if index < self.values.len() {
            self.values[index].as_ref()
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, index: I) -> Option<&mut V> {
        let index = index.to_usize();
        if index < self.values.len() {
            self.values[index].as_mut()
        } else {
            None
        }
    }

    pub fn remove(&mut self, index: I) -> Option<V> {
        let index = index.to_usize();
        if index < self.values.len() {
            self.values[index].take()
        } else {
            None
        }
    }

    pub fn contains(&self, index: I) -> bool {
        let index = index.to_usize();
        if index < self.values.len() {
            self.values[index].is_some()
        } else {
            false
        }
    }
}

impl<I: SparseIndex, V> Index<I> for SparseArray<I, V> {
    type Output = Option<V>;

    fn index(&self, index: I) -> &Self::Output {
        let index = index.to_usize();
        if index < self.values.len() {
            &self.values[index]
        } else {
            &None
        }
    }
}

impl<I: SparseIndex, V> IndexMut<I> for SparseArray<I, V> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        let index = index.to_usize();
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        &mut self.values[index]
    }
}

pub struct ImmutableSparseArray<I, V> {
    values: Box<[Option<V>]>,
    _marker: std::marker::PhantomData<I>,
}

impl<I, V> ImmutableSparseArray<I, V> {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<I: SparseIndex, V> ImmutableSparseArray<I, V> {
    pub fn get(&self, index: I) -> Option<&V> {
        let index = index.to_usize();
        self.values.get(index).and_then(|v| v.as_ref())
    }

    pub fn contains(&self, index: I) -> bool {
        let index = index.to_usize();
        self.values.get(index).is_some()
    }
}

impl<I, V> From<SparseArray<I, V>> for ImmutableSparseArray<I, V> {
    fn from(array: SparseArray<I, V>) -> Self {
        ImmutableSparseArray {
            values: array.values.into_boxed_slice(),
            _marker: std::marker::PhantomData,
        }
    }
}

pub struct SparseSet<I, V> {
    values: Vec<V>,
    indices: Vec<I>,
    sparse: SparseArray<I, usize>,
}

impl<I, V> SparseSet<I, V> {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            indices: Vec::new(),
            sparse: SparseArray::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[V] {
        &self.values
    }

    pub fn indices(&self) -> &[I] {
        &self.indices
    }

    pub fn iter(&self) -> impl Iterator<Item = (&I, &V)> {
        self.indices.iter().zip(self.values.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&I, &mut V)> {
        self.indices.iter().zip(self.values.iter_mut())
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.indices.clear();
        self.sparse.clear();
    }
}

impl<I: SparseIndex, V> FromIterator<(I, V)> for SparseSet<I, V> {
    fn from_iter<T: IntoIterator<Item = (I, V)>>(iter: T) -> Self {
        let mut set = SparseSet::new();
        for (index, value) in iter {
            set.insert(index, value);
        }
        set
    }
}

impl<I, V> Default for SparseSet<I, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: SparseIndex, V> SparseSet<I, V> {
    pub fn get(&self, index: I) -> Option<&V> {
        let index = self.sparse.get(index)?;
        Some(&self.values[*index])
    }

    pub fn get_mut(&mut self, index: I) -> Option<&mut V> {
        let index = self.sparse.get(index)?;
        Some(&mut self.values[*index])
    }

    pub fn insert(&mut self, index: I, value: V) -> Option<V> {
        if let Some(index) = self.sparse.get(index) {
            let value = std::mem::replace(&mut self.values[*index], value);
            Some(value)
        } else {
            self.sparse.insert(index, self.values.len());
            self.values.push(value);
            self.indices.push(index);
            None
        }
    }

    pub fn remove(&mut self, index: I) -> Option<V> {
        let index = self.sparse.remove(index)?;

        let value = self.values.swap_remove(index);
        self.indices.swap_remove(index);
        if index != self.values.len() {
            let last_index = self.indices[index];
            self.sparse.get_mut(last_index).map(|i| *i = index);
        }
        Some(value)
    }

    pub fn remove_at(&mut self, index: usize) -> Option<(I, V)> {
        if index >= self.values.len() {
            return None;
        }

        let value = self.values.swap_remove(index);
        let key = self.indices.swap_remove(index);
        if index != self.values.len() {
            let last_index = self.indices[index];
            self.sparse.get_mut(last_index).map(|i| *i = index);
        }
        Some((key, value))
    }

    pub fn contains(&self, index: I) -> bool {
        self.sparse.contains(index)
    }

    pub fn drain(&mut self) -> impl Iterator<Item = (I, V)> {
        self.indices
            .drain(..)
            .zip(self.values.drain(..))
            .map(|(index, value)| (index, value))
    }
}

pub type SparseSetIter<'a, I, V> = std::iter::Zip<std::slice::Iter<'a, I>, std::slice::Iter<'a, V>>;

pub struct ImmutableSparseSet<I, V> {
    values: Box<[V]>,
    indices: Box<[I]>,
    sparse: ImmutableSparseArray<I, usize>,
}

impl<I, V> ImmutableSparseSet<I, V> {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[V] {
        &self.values
    }

    pub fn indices(&self) -> &[I] {
        &self.indices
    }

    pub fn iter(&self) -> impl Iterator<Item = (&I, &V)> {
        self.indices.iter().zip(self.values.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&I, &mut V)> {
        self.indices.iter().zip(self.values.iter_mut())
    }
}

impl<I: SparseIndex, V> ImmutableSparseSet<I, V> {
    pub fn get(&self, index: I) -> Option<&V> {
        let index = self.sparse.get(index)?;
        Some(&self.values[*index])
    }

    pub fn get_mut(&mut self, index: I) -> Option<&mut V> {
        let index = self.sparse.get(index)?;
        Some(&mut self.values[*index])
    }

    pub fn contains(&self, index: I) -> bool {
        self.sparse.contains(index)
    }
}

impl<I, V> From<SparseSet<I, V>> for ImmutableSparseSet<I, V> {
    fn from(set: SparseSet<I, V>) -> Self {
        let values = set.values.into_boxed_slice();
        let indices = set.indices.into_boxed_slice();
        let sparse = ImmutableSparseArray::from(set.sparse);

        ImmutableSparseSet {
            values,
            indices,
            sparse,
        }
    }
}
