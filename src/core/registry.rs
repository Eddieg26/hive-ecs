use indexmap::IndexMap;
use std::{alloc::Layout, any::TypeId, collections::HashMap, ops::Index, sync::Arc};

pub type TypeMap<T> = HashMap<TypeId, T>;
pub type TypeIndexMap<T> = IndexMap<TypeId, T>;

pub trait Extension: downcast_rs::Downcast + Send + Sync + 'static {}
downcast_rs::impl_downcast!(Extension);

impl Extension for () {}

pub trait Object<Ext: Extension>: Sized + 'static {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u32);
impl std::ops::Deref for ObjectId {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<u32> for ObjectId {
    fn as_ref(&self) -> &u32 {
        &self.0
    }
}

pub struct Metadata {
    name: &'static str,
    id: ObjectId,
    layout: Layout,
    ext: Arc<dyn Extension>,
}

impl Metadata {
    pub fn new<T, Ext>(id: ObjectId, ext: Ext) -> Self
    where
        Ext: Extension,
        T: Object<Ext>,
    {
        let name = std::any::type_name::<T>();

        Self {
            name: if let Some(index) = name.rfind(":") {
                &name[index..]
            } else {
                name
            },
            id,
            layout: Layout::new::<T>(),
            ext: Arc::new(ext),
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn ext<T: Extension>(&self) -> &T {
        self.ext.downcast_ref::<T>().expect(&format!(
            "Invalid extension type: {}",
            std::any::type_name::<T>(),
        ))
    }
}

pub struct Registry(TypeIndexMap<Metadata>);

impl Registry {
    pub fn new() -> Self {
        Self(Default::default())
    }

    pub fn add<T, Ext>(&mut self, ext: Ext) -> ObjectId
    where
        Ext: Extension,
        T: Object<Ext>,
    {
        let ty = TypeId::of::<T>();
        match self.0.get(&ty) {
            Some(metadata) => metadata.id,
            None => {
                let id = ObjectId(self.0.len() as u32);
                let metadata = Metadata::new::<T, Ext>(id, ext);
                self.0.insert(ty, metadata);
                id
            }
        }
    }

    pub fn get<T, Ext>(&self) -> Option<&Metadata>
    where
        Ext: Extension,
        T: Object<Ext>,
    {
        self.0.get(&TypeId::of::<T>())
    }

    pub fn get_dyn(&self, ty: &TypeId) -> Option<&Metadata> {
        self.0.get(ty)
    }
}

impl Index<ObjectId> for Registry {
    type Output = Metadata;

    fn index(&self, index: ObjectId) -> &Self::Output {
        &self.0[index.0 as usize]
    }
}
