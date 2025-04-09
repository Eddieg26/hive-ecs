use crate::core::{
    blob::BlobCell,
    storage::{SparseArray, SparseIndex},
};
use std::{any::TypeId, collections::HashMap, thread::ThreadId};

pub trait Resource: Sized + 'static {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(u32);
impl SparseIndex for ResourceId {
    fn to_usize(self) -> usize {
        self.0 as usize
    }

    fn from_usize(index: usize) -> Self {
        Self(index as u32)
    }
}

pub struct ResourceCell {
    name: &'static str,
    data: BlobCell,
    owner: Option<ThreadId>,
    send: bool,
}

impl ResourceCell {
    pub fn new<const SEND: bool, R: Resource>(resource: R) -> Self {
        Self {
            name: std::any::type_name::<R>(),
            data: BlobCell::new(resource),
            owner: Some(std::thread::current().id()),
            send: SEND,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn get<R: Resource>(&self) -> &R {
        if !self.has_access(std::thread::current().id()) {
            panic!("Accessing non send resource from another thread is forbidden.")
        }

        self.data.value::<R>()
    }

    pub fn get_mut<R: Resource>(&mut self) -> &mut R {
        if !self.has_access(std::thread::current().id()) {
            panic!("Accessing non send resource from another thread is forbidden.")
        }

        self.data.value_mut::<R>()
    }

    pub fn into<R: Resource>(mut self) -> R {
        if !self.has_access(std::thread::current().id()) {
            panic!("Accessing non send resource from another thread is forbidden.")
        }

        let data = std::mem::take(&mut self.data);
        data.into()
    }

    pub fn owner(&self) -> Option<ThreadId> {
        self.owner
    }

    fn has_access(&self, id: ThreadId) -> bool {
        self.send || self.owner == Some(id)
    }
}

impl Drop for ResourceCell {
    fn drop(&mut self) {
        let id = std::thread::current().id();
        if !self.has_access(id) && !std::thread::panicking() {
            let name = self.name();
            let owner = self.owner();
            panic!(
                "Dopping a non-send resource {} that is owned by thread {:?} from thread {:?} is not allowed.",
                name, owner, id
            );
        }
    }
}

pub struct Resources {
    resources: SparseArray<ResourceId, ResourceCell>,
    map: HashMap<TypeId, ResourceId>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            resources: SparseArray::new(),
            map: HashMap::new(),
        }
    }

    pub fn register<const SEND: bool, R: Resource>(&mut self) -> ResourceId {
        let id = TypeId::of::<R>();
        match self.map.get(&id).copied() {
            Some(id) => id,
            None => {
                let resource_id = ResourceId(self.resources.len() as u32);
                self.map.insert(id, resource_id);
                self.resources.reserve(resource_id);
                resource_id
            }
        }
    }

    pub fn add<const SEND: bool, R: Resource>(&mut self, resource: R) -> ResourceId {
        let id = TypeId::of::<R>();
        match self.map.get(&id).copied() {
            Some(id) => id,
            None => {
                let resource_id = ResourceId(self.resources.len() as u32);
                self.map.insert(id, resource_id);
                self.resources
                    .insert(resource_id, ResourceCell::new::<SEND, R>(resource));
                resource_id
            }
        }
    }

    pub fn get<R: Resource>(&self) -> Option<&R> {
        let id = self.map.get(&TypeId::of::<R>())?;
        self.resources.get(*id).map(|cell| cell.get())
    }

    pub fn get_mut<R: Resource>(&mut self) -> Option<&mut R> {
        let id = self.map.get(&TypeId::of::<R>())?;
        self.resources.get_mut(*id).map(|cell| cell.get_mut())
    }

    pub fn contains<R: Resource>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<R>())
    }

    pub fn remove<R: Resource>(&mut self) -> Option<R> {
        let id = self.map.get(&TypeId::of::<R>())?;
        self.resources.remove(*id).map(|r| r.into())
    }
}

pub struct Res<'a, R: Resource>(&'a R);
impl<'a, R: Resource> Res<'a, R> {
    pub fn new(resource: &'a R) -> Self {
        Self(resource)
    }
}

impl<'a, R: Resource> std::ops::Deref for Res<'a, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, R: Resource> AsRef<R> for Res<'a, R> {
    fn as_ref(&self) -> &R {
        self.0
    }
}

pub struct ResMut<'a, R: Resource>(&'a mut R);
impl<'a, R: Resource> ResMut<'a, R> {
    pub fn new(resource: &'a mut R) -> Self {
        Self(resource)
    }
}

impl<'a, R: Resource> std::ops::Deref for ResMut<'a, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, R: Resource> std::ops::DerefMut for ResMut<'a, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<'a, R: Resource> AsRef<R> for ResMut<'a, R> {
    fn as_ref(&self) -> &R {
        self.0
    }
}

impl<'a, R: Resource> AsMut<R> for ResMut<'a, R> {
    fn as_mut(&mut self) -> &mut R {
        self.0
    }
}

pub struct NonSend<'a, R: Resource>(&'a R);
impl<'a, R: Resource> NonSend<'a, R> {
    pub fn new(resource: &'a R) -> Self {
        Self(resource)
    }
}

impl<'a, R: Resource> std::ops::Deref for NonSend<'a, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, R: Resource> AsRef<R> for NonSend<'a, R> {
    fn as_ref(&self) -> &R {
        self.0
    }
}

pub struct NonSendMut<'a, R: Resource>(&'a mut R);
impl<'a, R: Resource> NonSendMut<'a, R> {
    pub fn new(resource: &'a mut R) -> Self {
        Self(resource)
    }
}

impl<'a, R: Resource> std::ops::Deref for NonSendMut<'a, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, R: Resource> std::ops::DerefMut for NonSendMut<'a, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<'a, R: Resource> AsRef<R> for NonSendMut<'a, R> {
    fn as_ref(&self) -> &R {
        self.0
    }
}

impl<'a, R: Resource> AsMut<R> for NonSendMut<'a, R> {
    fn as_mut(&mut self) -> &mut R {
        self.0
    }
}

pub struct Cloned<R: Resource>(R);
impl<R: Resource> Cloned<R> {
    pub fn new(resource: R) -> Self {
        Self(resource)
    }
}
impl<R: Resource> std::ops::Deref for Cloned<R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R: Resource> AsRef<R> for Cloned<R> {
    fn as_ref(&self) -> &R {
        &self.0
    }
}

impl<R: Resource> std::ops::DerefMut for Cloned<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<R: Resource> AsMut<R> for Cloned<R> {
    fn as_mut(&mut self) -> &mut R {
        &mut self.0
    }
}

impl<R: Resource + Clone> Clone for Cloned<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
