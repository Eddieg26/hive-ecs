use std::{alloc::Layout, marker::PhantomData};

pub struct Blob {
    name: &'static str,
    data: Vec<u8>,
    length: usize,
    capacity: usize,
    layout: Layout,
    aligned_layout: Layout,
    drop: Option<fn(data: *mut u8)>,
}

impl Default for Blob {
    fn default() -> Self {
        Self::empty()
    }
}

impl Blob {
    pub fn new<T: 'static>(capacity: usize) -> Self {
        let layout = Layout::new::<T>();
        let aligned_layout = layout.pad_to_align();
        let data = Vec::with_capacity(aligned_layout.size() * capacity);

        let drop = match std::mem::needs_drop::<T>() {
            true => Some(drop::<T> as fn(*mut u8)),
            false => None,
        };

        Self {
            name: std::any::type_name::<T>(),
            data,
            capacity,
            length: 0,
            layout,
            aligned_layout,
            drop,
        }
    }

    pub fn empty() -> Self {
        Self {
            name: "",
            data: Vec::new(),
            length: 0,
            capacity: 0,
            layout: Layout::new::<u8>(),
            aligned_layout: Layout::new::<u8>(),
            drop: None,
        }
    }

    pub fn from_value<T: 'static>(value: T) -> Self {
        let layout = Layout::new::<T>();
        let aligned_layout = layout.pad_to_align();
        let mut data = Vec::with_capacity(aligned_layout.size() * 2);
        unsafe {
            std::ptr::write(data.as_mut_ptr() as *mut T, value);
            data.set_len(aligned_layout.size());
        }

        let drop = match std::mem::needs_drop::<T>() {
            true => Some(drop::<T> as fn(*mut u8)),
            false => None,
        };

        Self {
            name: std::any::type_name::<T>(),
            data,
            capacity: 2,
            length: 1,
            layout,
            aligned_layout,
            drop,
        }
    }

    pub fn from_cell(mut cell: BlobCell) -> Self {
        let data = std::mem::take(&mut cell.data);
        let layout = cell.layout;
        let drop = cell.drop.take();
        let aligned_layout = layout.pad_to_align();
        let name = cell.name;

        Self {
            name,
            data,
            length: 1,
            capacity: 1,
            layout,
            aligned_layout,
            drop,
        }
    }

    pub fn with_layout(layout: Layout, capacity: usize, drop: Option<fn(*mut u8)>) -> Self {
        let aligned_layout = layout.pad_to_align();
        let data = Vec::with_capacity(capacity * aligned_layout.size());

        Self {
            name: "",
            data,
            capacity,
            length: 0,
            layout,
            aligned_layout,
            drop,
        }
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn aligned_layout(&self) -> &Layout {
        &self.aligned_layout
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn drop(&self) -> Option<&fn(*mut u8)> {
        self.drop.as_ref()
    }

    pub fn get<T: 'static>(&self, index: usize) -> Option<&T> {
        if index < self.length {
            Some(unsafe { &*(self.offset(index) as *const T) })
        } else {
            None
        }
    }

    pub fn get_mut<T>(&mut self, index: usize) -> Option<&mut T> {
        if index < self.length {
            Some(unsafe { &mut *(self.offset(index) as *mut T) })
        } else {
            None
        }
    }

    pub unsafe fn get_mut_unsafe<T: 'static>(&self, index: usize) -> Option<&mut T> {
        if index < self.length {
            Some(unsafe { &mut *(self.offset(index) as *mut T) })
        } else {
            None
        }
    }

    pub fn push<T: 'static>(&mut self, value: T) {
        if self.length == self.capacity {
            self.reserve(self.capacity.max(1));
        }

        unsafe {
            let dst = self.offset(self.length) as *mut T;
            std::ptr::write(dst, value);

            self.length += 1;
            self.data.set_len(self.length * self.aligned_layout.size());
        }
    }

    pub fn insert<T: 'static>(&mut self, index: usize, value: T) {
        if index >= self.length {
            panic!("Index out of bounds.")
        }

        if self.length == self.capacity {
            self.reserve(self.capacity.max(1));
        }

        unsafe {
            let src = self.offset(index);
            let dst = self.offset(index + 1);

            std::ptr::copy(src, dst, self.length - index);
            std::ptr::write(src as *mut T, value);

            self.length += 1;
            self.data.set_len(self.length * self.aligned_layout.size());
        }
    }

    pub fn remove<T: 'static>(&mut self, index: usize) -> T {
        if index >= self.length {
            panic!("Index out of bounds.")
        }

        unsafe {
            let src = self.offset(index) as *const T;
            let value = std::ptr::read(src);

            if index + 1 < self.length {
                let dst = src as *mut u8;
                let src = self.offset(index + 1);
                let count = self.length - (index + 1) * self.aligned_layout.size();
                std::ptr::copy(src, dst, count);
            }

            self.length -= 1;
            self.data.set_len(self.length * self.aligned_layout.size());

            if self.length < self.capacity / 2 {
                self.shrink(self.capacity / 2);
            }

            value
        }
    }

    pub fn swap_remove<T: 'static>(&mut self, index: usize) -> T {
        if index >= self.length {
            panic!("Index out of bounds.")
        }

        unsafe {
            let src = self.offset(index) as *const T;
            let value = std::ptr::read(src);

            if index < self.length {
                let dst = src as *mut u8;
                let src = self.offset(self.length - 1);
                std::ptr::copy(src, dst, self.aligned_layout.size());
            }

            self.length -= 1;
            self.data.set_len(self.length * self.aligned_layout.size());

            if self.length < self.capacity / 2 {
                self.shrink(self.capacity / 2);
            }

            value
        }
    }

    pub fn append<T: 'static>(&mut self, iter: impl IntoIterator<Item = T>) {
        for value in iter.into_iter() {
            self.push(value)
        }
    }

    pub fn extend(&mut self, mut blob: Blob) {
        if blob.aligned_layout != self.aligned_layout || blob.layout != self.layout {
            panic!(
                "Layouts are different: {:?} != {:?}",
                blob.layout, self.layout
            )
        }

        self.reserve(blob.length);
        let data = &blob.data[..blob.length * blob.aligned_layout.size()];
        self.data.extend(data);
        self.length += blob.length;
        unsafe {
            self.data.set_len(self.length * self.aligned_layout.size());
        }

        blob.length = 0;
        blob.capacity = 0;
    }

    pub fn push_blob(&mut self, mut blob: Blob) {
        if blob.aligned_layout != self.aligned_layout || blob.layout != self.layout {
            panic!("Layouts are different")
        }

        self.reserve(blob.length);
        self.data.append(&mut blob.data);
        self.length += blob.length;

        blob.length = 0;
        blob.capacity = 0;
    }

    pub fn insert_blob(&mut self, index: usize, blob: Blob) {
        if blob.aligned_layout != self.aligned_layout || blob.layout != self.layout {
            panic!("Layouts are different")
        }

        if index >= self.length {
            panic!("Index out of bounds.")
        }

        self.reserve(blob.capacity);
        unsafe {
            if self.length > 1 {
                let src = self.offset(index);
                let dst = self.offset(index + blob.length);
                let count = (self.capacity - index) * self.aligned_layout.size();
                std::ptr::copy(src, dst, count);
            }

            let count = blob.length * self.aligned_layout.size();
            std::ptr::copy(blob.offset(0), self.offset(index), count);

            self.length += blob.length;
            self.data.set_len(self.length * self.aligned_layout.size());
        }
    }

    pub fn remove_blob(&mut self, index: usize) -> Blob {
        if index >= self.length {
            panic!("Index out of bounds.")
        }

        let start = index * self.aligned_layout.size();
        let end = start + self.aligned_layout().size();
        let data = self.data.drain(start..end).collect::<Vec<_>>();

        self.length -= 1;
        unsafe {
            self.data.set_len(self.length * self.aligned_layout.size());
        }

        if self.length < self.capacity / 2 {
            self.shrink(self.capacity / 2);
        }

        Blob {
            name: self.name,
            aligned_layout: self.aligned_layout,
            layout: self.layout,
            drop: self.drop.clone(),
            capacity: 1,
            length: 1,
            data,
        }
    }

    pub fn swap_remove_blob(&mut self, index: usize) -> Blob {
        if index >= self.length {
            panic!("Index out of bounds.")
        }

        let start = (self.length - 1) * self.aligned_layout.size();
        let end = start + self.aligned_layout.size();
        let mut data = self.data.drain(start..end).collect::<Vec<_>>();

        if self.length > 1 {
            let start = index * self.aligned_layout.size();
            let end = start + self.aligned_layout().size();

            data = self.data.splice(start..end, data).collect::<Vec<_>>();
        }

        self.length -= 1;
        unsafe {
            self.data.set_len(self.length * self.aligned_layout.size());
        }

        if self.length < self.capacity / 2 {
            self.shrink(self.capacity / 2);
        }

        Blob {
            name: self.name,
            aligned_layout: self.aligned_layout,
            layout: self.layout,
            drop: self.drop.clone(),
            capacity: 1,
            length: 1,
            data,
        }
    }

    pub fn push_cell(&mut self, mut cell: BlobCell) {
        if cell.layout != self.layout {
            panic!("Layouts are different")
        }

        if self.length == self.capacity {
            self.reserve(self.capacity.max(1));
        }

        unsafe {
            let dst = self.offset(self.length) as *mut u8;
            std::ptr::copy(cell.data.as_ptr(), dst, cell.layout.size());

            self.length += 1;
            self.data.set_len(self.length * self.aligned_layout.size());
        }

        cell.data = vec![];
        cell.drop = None;
    }

    pub fn insert_cell(&mut self, index: usize, mut cell: BlobCell) {
        if cell.layout != self.layout {
            panic!("Layouts are different")
        }

        if index >= self.length {
            panic!("Index out of bounds.")
        }

        if self.length == self.capacity {
            self.reserve(self.capacity.max(1));
        }

        unsafe {
            let src = self.offset(index);
            let dst = self.offset(index + 1);

            std::ptr::copy(src, dst, self.length - index);
            std::ptr::copy(cell.data.as_ptr(), src as *mut u8, cell.layout.size());

            self.length += 1;
            self.data.set_len(self.length * self.aligned_layout.size());
        }

        cell.data = vec![];
        cell.drop = None;
    }

    pub fn remove_cell(&mut self, index: usize) -> BlobCell {
        self.remove_cell_checked(index)
            .unwrap_or_else(|| panic!("Index out of bounds."))
    }

    pub fn remove_cell_checked(&mut self, index: usize) -> Option<BlobCell> {
        if index >= self.length {
            return None;
        }

        let start = index * self.aligned_layout.size();
        let end = start + self.aligned_layout.size();
        let data = self.data.drain(start..end).collect::<Vec<_>>();

        self.length -= 1;

        if self.length < self.capacity / 2 {
            self.shrink(self.capacity / 2);
        }

        Some(BlobCell {
            name: self.name,
            data,
            layout: self.layout,
            drop: self.drop.clone(),
        })
    }

    pub fn swap_remove_cell_checked(&mut self, index: usize) -> Option<BlobCell> {
        if index >= self.length {
            return None;
        }

        let start = (self.length - 1) * self.aligned_layout.size();
        let end = start + self.aligned_layout.size();
        let mut data = self.data.drain(start..end).collect::<Vec<_>>();

        if self.length > 1 {
            let start = index * self.aligned_layout.size();
            let end = start + self.aligned_layout().size();

            data = self.data.splice(start..end, data).collect::<Vec<_>>();
        }

        self.length -= 1;

        if self.length < self.capacity / 2 {
            self.shrink(self.capacity / 2);
        }

        Some(BlobCell {
            name: self.name,
            data,
            layout: self.layout,
            drop: self.drop.clone(),
        })
    }

    pub fn swap_remove_cell(&mut self, index: usize) -> BlobCell {
        self.swap_remove_cell_checked(index)
            .unwrap_or_else(|| panic!("Index out of bounds."))
    }

    pub fn clear(&mut self) {
        if let Some(drop) = self.drop {
            for index in 0..self.length {
                drop(self.offset(index))
            }
        }

        self.data.clear();
        self.length = 0;
        self.capacity = 0;
    }

    pub fn iter<T: 'static>(&self) -> BlobIter<T> {
        BlobIter::<T>::new(self)
    }

    pub fn iter_mut<T: 'static>(&mut self) -> BlobIterMut<T> {
        BlobIterMut::<T>::new(self)
    }

    /// Allows for shared access to the blob data.
    pub unsafe fn ptr<T: 'static>(&self, index: usize) -> Ptr<T> {
        if index >= self.length {
            panic!("Index out of bounds.")
        }

        Ptr::new(self.offset(index) as *mut T)
    }

    pub fn reserve(&mut self, additional: usize) {
        self.data
            .reserve_exact(additional * self.aligned_layout.size());

        self.capacity = self.data.capacity() / self.aligned_layout.size().clamp(1, usize::MAX);
    }

    pub fn shrink(&mut self, min_capacity: usize) {
        self.data
            .shrink_to(min_capacity * self.aligned_layout.size());

        self.capacity = self.data.capacity() / self.aligned_layout.size().clamp(1, usize::MAX);
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }
}

impl Blob {
    fn offset(&self, offset: usize) -> *mut u8 {
        let count: isize = (offset * self.aligned_layout.size()).try_into().unwrap();
        let bounds: isize = (self.capacity * self.aligned_layout.size()
            - self.aligned_layout.size())
        .try_into()
        .unwrap();
        if count > bounds {
            panic!("Index out of bounds.")
        }
        unsafe { self.data.as_ptr().offset(count) as *mut u8 }
    }
}

impl Drop for Blob {
    fn drop(&mut self) {
        self.clear()
    }
}

pub struct BlobCell {
    name: &'static str,
    data: Vec<u8>,
    layout: Layout,
    drop: Option<fn(data: *mut u8)>,
}

impl Default for BlobCell {
    fn default() -> Self {
        Self::empty()
    }
}

impl BlobCell {
    pub fn new<T: 'static>(value: T) -> Self {
        let layout = Layout::new::<T>();
        let data = unsafe {
            let ptr = std::ptr::addr_of!(value) as *mut u8;
            let mut data = Vec::with_capacity(layout.size());
            data.set_len(layout.size());
            std::ptr::copy(ptr, data.as_mut_ptr(), layout.size());
            std::mem::forget(value);
            data
        };

        let drop = match std::mem::needs_drop::<T>() {
            true => Some(drop::<T> as fn(*mut u8)),
            false => None,
        };

        Self {
            name: std::any::type_name::<T>(),
            data,
            layout,
            drop,
        }
    }

    pub fn from_blob(mut blob: Blob) -> Self {
        if blob.length != 1 {
            panic!("Blob length must be 1.")
        }

        let mut data = std::mem::take(&mut blob.data);
        let layout = blob.layout;
        let drop = blob.drop;
        blob.length = 0;
        blob.capacity = 0;

        unsafe {
            data.set_len(layout.size());
        }

        Self {
            name: blob.name,
            data,
            layout,
            drop,
        }
    }

    pub fn empty() -> Self {
        Self {
            name: "",
            data: Vec::new(),
            layout: Layout::new::<u8>(),
            drop: None,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn drop(&self) -> Option<&fn(*mut u8)> {
        self.drop.as_ref()
    }

    pub fn value<T: 'static>(&self) -> &T {
        unsafe { &*(self.data.as_ptr() as *const T) }
    }

    pub fn value_mut<T: 'static>(&mut self) -> &mut T {
        unsafe { &mut *(self.data.as_ptr() as *mut T) }
    }

    pub fn value_checked<T: 'static>(&self) -> Option<&T> {
        if self.layout.size() == std::mem::size_of::<T>() {
            Some(unsafe { &*(self.data.as_ptr() as *const T) })
        } else {
            None
        }
    }

    pub fn value_mut_checked<T: 'static>(&mut self) -> Option<&mut T> {
        if self.layout.size() == std::mem::size_of::<T>() {
            Some(unsafe { &mut *(self.data.as_ptr() as *mut T) })
        } else {
            None
        }
    }

    pub unsafe fn value_mut_unsafe<T: 'static>(&self) -> &mut T {
        unsafe { &mut *(self.data.as_ptr() as *mut T) }
    }

    pub fn ptr<T: 'static>(&self) -> Ptr<T> {
        Ptr::new(self.data.as_ptr() as *mut T)
    }

    pub fn into<T: 'static>(self) -> T {
        unsafe {
            let value = (self.data.as_ptr() as *const T).read();
            std::mem::forget(self);
            value
        }
    }
}

impl Drop for BlobCell {
    fn drop(&mut self) {
        if let Some(drop) = self.drop {
            drop(self.data.as_mut_ptr());
        }

        self.data.clear();
    }
}

impl<T: 'static> From<Vec<T>> for Blob {
    fn from(value: Vec<T>) -> Self {
        let mut blob = Blob::new::<T>(value.capacity());
        blob.append(value);
        blob
    }
}

fn drop<T>(data: *mut u8) {
    unsafe {
        let raw = data as *mut T;
        std::mem::drop(raw.read());
    }
}

pub struct Ptr<'a, T: 'static> {
    data: *mut T,
    _marker: PhantomData<&'a T>,
}

impl<'a, T: 'static> Ptr<'a, T> {
    pub fn new(data: *mut T) -> Self {
        Self {
            data,
            _marker: Default::default(),
        }
    }

    pub fn get(&self, index: usize) -> Option<&'a T> {
        if index < std::mem::size_of::<T>() {
            Some(unsafe { &*self.data.add(index) })
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&'a mut T> {
        if index < std::mem::size_of::<T>() {
            Some(unsafe { &mut *self.data.add(index) })
        } else {
            None
        }
    }
}

pub struct BlobIter<'a, T: 'static> {
    blob: &'a Blob,
    index: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: 'static> BlobIter<'a, T> {
    fn new(blob: &'a Blob) -> Self {
        Self {
            blob,
            index: 0,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: 'static> Iterator for BlobIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.blob.get::<T>(self.index);
        self.index += 1;
        value
    }
}

pub struct BlobIterMut<'a, T: 'static> {
    blob: &'a mut Blob,
    index: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: 'static> BlobIterMut<'a, T> {
    fn new(blob: &'a mut Blob) -> Self {
        Self {
            blob,
            index: 0,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: 'static> Iterator for BlobIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        let blob_ptr: *mut Blob = self.blob;

        unsafe {
            // SAFETY: We ensure that we do not create multiple mutable references from this iterator.
            let blob_mut: &mut Blob = &mut *blob_ptr;
            let value = blob_mut.get_mut::<T>(self.index);

            self.index += 1;
            value
        }
    }
}

pub mod v2 {
    use std::{
        alloc::Layout,
        ptr::{self},
    };

    #[derive(Debug, Clone, Copy)]
    pub struct TypeMeta {
        pub name: &'static str,
        pub layout: Layout,
        pub drop: Option<fn(data: *mut u8)>,
    }

    impl TypeMeta {
        pub fn new<T: 'static>() -> Self {
            Self {
                name: std::any::type_name::<T>(),
                layout: Layout::new::<T>(),
                drop: match std::mem::needs_drop::<T>() {
                    true => Some(Self::drop::<T> as fn(*mut u8)),
                    false => None,
                },
            }
        }

        fn drop<T>(data: *mut u8) {
            unsafe {
                let raw = data as *mut T;
                std::mem::drop(raw.read());
            }
        }
    }

    pub struct Blob {
        data: Vec<u8>,
        meta: TypeMeta,
    }

    impl Blob {
        pub fn new<T: 'static>() -> Self {
            let meta = TypeMeta::new::<T>();

            Self { data: vec![], meta }
        }

        pub unsafe fn from_raw(data: Vec<u8>, meta: TypeMeta) -> Self {
            Self { data, meta }
        }

        pub fn with_meta(meta: TypeMeta) -> Self {
            Self { data: vec![], meta }
        }

        pub fn data(&self) -> &[u8] {
            &self.data
        }

        pub fn meta(&self) -> &TypeMeta {
            &self.meta
        }

        pub fn get<T: 'static>(&self, index: usize) -> Option<&T> {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            let offset = index * self.meta.layout.size();
            if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
                return None;
            }

            unsafe { (self.data.as_ptr().add(offset) as *const T).as_ref() }
        }

        pub fn get_mut<T: 'static>(&mut self, index: usize) -> Option<&mut T> {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            let offset = index * self.meta.layout.size();
            if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
                return None;
            }

            unsafe { (self.data.as_mut_ptr().add(offset) as *mut T).as_mut() }
        }

        pub fn push<T: 'static>(&mut self, value: T) {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            let offset = self.data.len();
            self.data
                .resize(self.data.len() + self.meta.layout.size(), 0);

            unsafe {
                let dst = self.data.as_mut_ptr().add(offset);
                ptr::write(dst as *mut T, value);
            };
        }

        pub fn insert<T: 'static>(&mut self, index: usize, value: T) {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            let offset = index * self.meta.layout.size();
            let bounds = self.data.len() - self.meta.layout.size();
            if offset > bounds {
                panic!("Index out of bounds: {}", index);
            }
            self.data
                .resize(self.data.len() + self.meta.layout.size(), 0);

            unsafe {
                let src = self.data.as_ptr().add(offset);
                let dst = self.data.as_mut_ptr().add(offset + self.meta.layout.size());

                ptr::copy(src, dst, self.data.len() - offset);
                ptr::write(src as *mut T, value);
            }
        }

        pub fn append<T: 'static>(&mut self, values: Vec<T>) {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            let offset = self.data.len();
            self.data
                .resize(offset + self.meta.layout.size() * values.len(), 0);

            unsafe {
                let src = values.as_ptr() as *mut T;
                let dst = self.data.as_mut_ptr().add(offset) as *mut T;

                ptr::copy_nonoverlapping(src, dst, values.len());

                std::mem::forget(values);
            }
        }

        pub fn remove<T: 'static>(&mut self, index: usize) -> T {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            let offset = index * self.meta.layout.size();
            if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
                panic!("Index out of bounds: {}", index);
            }

            unsafe {
                let src = self.data.as_ptr().add(offset) as *const T;
                let value = ptr::read::<T>(src);

                self.data.drain(offset..offset + self.meta.layout.size());

                value
            }
        }

        pub fn swap_remove<T: 'static>(&mut self, index: usize) -> T {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            let offset = index * self.meta.layout.size();
            let bounds = self.data.len() - self.meta.layout.size();

            if offset > bounds {
                panic!("Index out of bounds: {}", index);
            }

            unsafe {
                let dst = self.data.as_mut_ptr().add(offset) as *mut T;
                let src = self.data.as_ptr().add(bounds) as *const T;

                let value = ptr::read(dst);
                if offset != bounds {
                    ptr::copy_nonoverlapping(src, dst, 1);
                }

                self.data.set_len(bounds);

                value
            }
        }

        pub unsafe fn append_raw(&mut self, value: Vec<u8>) {
            assert!(value.len() % self.meta.layout.size() == 0);

            self.data.extend(value);
        }

        pub unsafe fn insert_raw(&mut self, index: usize, value: Vec<u8>) {
            let offset = index * self.meta.layout.size();
            if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
                panic!("Index out of bounds: {}", index);
            }
            self.data.resize(self.data.len() + value.len(), 0);

            unsafe {
                let src = self.data.as_ptr().add(offset);
                let dst = self.data.as_mut_ptr().add(offset + self.meta.layout.size());

                ptr::copy(src, dst, self.data.len() - offset);
                ptr::copy_nonoverlapping(value.as_ptr(), src as *mut u8, value.len());
            }
        }

        pub unsafe fn remove_raw(&mut self, index: usize) -> Vec<u8> {
            let offset = index * self.meta.layout.size();
            if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
                panic!("Index out of bounds: {}", index);
            }

            self.data
                .drain(offset..offset + self.meta.layout.size())
                .collect()
        }

        pub unsafe fn swap_remove_raw(&mut self, index: usize) -> Vec<u8> {
            let offset = index * self.meta.layout.size();
            if self.data.is_empty() || offset > self.data.len() - self.meta.layout.size() {
                panic!("Index out of bounds: {}", index);
            }

            unsafe {
                let mut bytes = vec![0u8; self.meta.layout.size()];
                let src = self
                    .data
                    .as_ptr()
                    .add(self.data.len() - self.meta.layout.size());
                ptr::copy_nonoverlapping(src, bytes.as_mut_ptr(), bytes.len());

                let bytes = self
                    .data
                    .splice(offset..offset + self.meta.layout.size(), bytes)
                    .collect::<Vec<_>>();

                self.data.set_len(self.data.len() - self.meta.layout.size());

                bytes
            }
        }

        pub fn len(&self) -> usize {
            self.data.len() / self.meta.layout.size()
        }

        pub fn is_empty(&self) -> bool {
            self.data.len() == 0
        }

        pub fn clear(&mut self) {
            self.data.clear();
        }

        pub fn into_raw(mut self) -> (Vec<u8>, TypeMeta) {
            (std::mem::take(&mut self.data), self.meta)
        }

        pub fn to_vec<T: 'static>(self) -> Vec<T> {
            unsafe {
                let values = Vec::from_raw_parts(
                    self.data.as_ptr() as *mut T,
                    self.len(),
                    self.data.capacity() / self.meta.layout.size(),
                );

                std::mem::forget(self);

                values
            }
        }
    }

    impl Drop for Blob {
        fn drop(&mut self) {
            if let Some(drop) = self.meta.drop {
                for index in 0..self.len() {
                    let offset = index * self.meta.layout.size();
                    let value = unsafe { self.data.as_mut_ptr().add(offset) };
                    drop(value);
                }
            }

            self.data.clear();
        }
    }

    impl From<BlobCell> for Blob {
        fn from(value: BlobCell) -> Self {
            let blob = Self {
                data: unsafe {
                    Vec::from_raw_parts(
                        value.data.as_ptr() as *mut u8,
                        value.data.len(),
                        value.data.capacity(),
                    )
                },
                meta: value.meta,
            };

            std::mem::forget(value);

            blob
        }
    }

    // impl Into<Blob> for BlobCell {
    //     fn into(self) -> Blob {
    //         todo!()
    //     }
    // }

    pub struct BlobCell {
        data: Vec<u8>,
        meta: TypeMeta,
    }

    impl BlobCell {
        pub fn new<T: 'static>(value: T) -> Self {
            let meta = TypeMeta::new::<T>();
            let mut data = vec![0u8; meta.layout.size()];

            unsafe { ptr::write(data.as_mut_ptr() as *mut T, value) };

            Self { data, meta }
        }

        pub unsafe fn from_raw(data: Vec<u8>, meta: TypeMeta) -> Self {
            Self { data, meta }
        }

        pub fn data(&self) -> &[u8] {
            &self.data
        }

        pub fn meta(&self) -> &TypeMeta {
            &self.meta
        }

        pub fn get<T: 'static>(&self) -> &T {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            unsafe { (self.data.as_ptr() as *const T).as_ref().unwrap() }
        }

        pub fn get_mut<T: 'static>(&mut self) -> &mut T {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            unsafe { (self.data.as_mut_ptr() as *mut T).as_mut().unwrap() }
        }

        pub fn into_raw(mut self) -> (Vec<u8>, TypeMeta) {
            let data = std::mem::take(&mut self.data);
            let meta = self.meta;

            std::mem::forget(self);

            (data, meta)
        }

        pub fn into_value<T: 'static>(self) -> T {
            assert_eq!(std::mem::size_of::<T>(), self.meta.layout.size());

            let value = unsafe { std::ptr::read(self.data.as_ptr() as *const T) };

            std::mem::forget(self);

            value
        }
    }

    impl Drop for BlobCell {
        fn drop(&mut self) {
            if let Some(drop) = self.meta.drop {
                let value = self.data.as_mut_ptr();
                drop(value);
            }

            self.data.clear();
        }
    }

    #[allow(unused_imports)]
    mod tests {
        use super::{Blob, BlobCell, TypeMeta};

        #[test]
        fn blob_from_raw() {
            let values = [10, 20, 30, 40];
            let mut bytes = vec![0u8; std::mem::size_of::<i32>() * 4];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    values.as_ptr() as *const u8,
                    bytes.as_mut_ptr(),
                    std::mem::size_of::<i32>() * 4,
                )
            };

            let meta = TypeMeta::new::<i32>();

            let blob = unsafe { Blob::from_raw(bytes, meta) };
            for (index, value) in values.iter().enumerate() {
                assert_eq!(blob.get::<i32>(index), Some(value));
            }
        }

        #[test]
        fn blob_push_and_get() {
            let mut blob = Blob::new::<i32>();
            blob.push(10);
            blob.push(20);
            blob.push(30);

            assert_eq!(blob.get(0), Some(&10));
            assert_eq!(blob.get(1), Some(&20));
            assert_eq!(blob.get(2), Some(&30));
        }

        #[test]
        fn blob_insert_and_get_mut() {
            let mut blob = Blob::new::<i32>();
            blob.push(10);
            blob.push(30);
            blob.push(40);
            blob.insert(1, 20);

            assert_eq!(blob.get(0), Some(&10));
            assert_eq!(blob.get(1), Some(&20));
            assert_eq!(blob.get(2), Some(&30));
            assert_eq!(blob.get_mut(3), Some(&mut 40));
        }

        #[test]
        fn blob_append() {
            let values = vec![10, 20, 30, 40];
            let mut blob = Blob::new::<i32>();
            blob.append(values.clone());

            for (index, value) in values.iter().enumerate() {
                assert_eq!(blob.get::<i32>(index), Some(value));
            }
        }

        #[test]
        fn blob_remove() {
            let mut blob = Blob::new::<i32>();
            blob.push(10);
            blob.push(20);

            let value = blob.remove::<i32>(1);
            assert_eq!(value, 20);

            let value = blob.remove::<i32>(0);
            assert_eq!(value, 10);
        }

        #[test]
        fn blob_swap_remove() {
            let mut blob = Blob::new::<i32>();
            blob.push(10);
            blob.push(20);
            blob.push(30);

            let value = blob.swap_remove::<i32>(0);
            assert_eq!(value, 10);

            let value = blob.get::<i32>(0);
            assert_eq!(value, Some(&30));
        }

        #[test]
        fn blob_append_raw() {
            let values = [10, 20, 30, 40];
            let mut bytes = vec![0u8; std::mem::size_of::<i32>() * 4];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    values.as_ptr() as *const u8,
                    bytes.as_mut_ptr(),
                    std::mem::size_of::<i32>() * 4,
                )
            };

            let mut blob = Blob::new::<i32>();
            unsafe { blob.append_raw(bytes) };

            for (index, value) in values.iter().enumerate() {
                assert_eq!(blob.get::<i32>(index), Some(value));
            }
        }

        #[test]
        fn blob_insert_raw() {
            let value = 20;
            let mut bytes = vec![0u8; std::mem::size_of::<i32>()];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    std::ptr::addr_of!(value) as *const u8,
                    bytes.as_mut_ptr(),
                    std::mem::size_of::<i32>(),
                )
            };

            let mut blob = Blob::new::<i32>();
            blob.push(10);
            blob.push(30);
            blob.push(40);

            unsafe { blob.insert_raw(1, bytes) };
            assert_eq!(blob.get(0), Some(&10));
            assert_eq!(blob.get(1), Some(&20));
            assert_eq!(blob.get(2), Some(&30));
            assert_eq!(blob.get(3), Some(&40));
        }

        #[test]
        fn blob_remove_raw() {
            let mut blob = Blob::new::<i32>();
            blob.push(10);

            let bytes = unsafe { blob.remove_raw(0) };
            let value = unsafe { (bytes.as_ptr() as *const i32).as_ref().unwrap() };
            assert_eq!(value, &10);
        }

        #[test]
        fn blob_swap_remove_raw() {
            let mut blob = Blob::new::<i32>();
            blob.push(10);
            blob.push(20);
            blob.push(30);

            let bytes = unsafe { blob.swap_remove_raw(0) };
            let value = unsafe { (bytes.as_ptr() as *const i32).as_ref().unwrap() };
            assert_eq!(value, &10);
            assert_eq!(blob.get(0), Some(&30));
        }

        #[test]
        fn blob_to_vec() {
            let values = vec![10, 20, 30, 40];
            let mut blob = Blob::new::<i32>();
            blob.append(values.clone());

            assert_eq!(values, blob.to_vec::<i32>());
        }

        #[test]
        fn blob_from_blob_cell() {
            let cell = BlobCell::new(10);
            let blob = Blob::from(cell);

            assert_eq!(blob.get(0), Some(&10));
        }

        #[test]
        fn blob_cell_into_blob() {
            let cell = BlobCell::new(10);
            let blob: Blob = cell.into();

            assert_eq!(blob.get(0), Some(&10));
        }

        #[test]
        fn blob_cell_new() {
            let blob = BlobCell::new(10);

            assert_eq!(blob.get::<i32>(), &10);
        }

        #[test]
        fn blob_cell_from_raw() {
            let value = 10;
            let mut bytes = vec![0u8; std::mem::size_of::<i32>()];
            unsafe {
                std::ptr::write(bytes.as_mut_ptr() as *mut i32, value);
            }

            let meta = TypeMeta::new::<i32>();
            let blob = unsafe { BlobCell::from_raw(bytes, meta) };

            assert_eq!(blob.get::<i32>(), &10);
        }

        #[test]
        fn blob_cell_into_value() {
            let blob = BlobCell::new(10);

            assert_eq!(blob.into_value::<i32>(), 10);
        }
    }
}
