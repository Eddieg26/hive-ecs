use super::{
    blob::BlobCell,
    registry::{Object, ObjectId},
};
use indexmap::IndexMap;
use std::thread::ThreadId;

pub trait Resource: Sized + 'static {}

impl<R: Resource> Object<()> for R {}

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

pub struct ResourceStorage(IndexMap<ObjectId, ResourceCell>);

impl ResourceStorage {
    pub fn new() -> Self {
        Self(IndexMap::new())
    }

    pub fn add<const SEND: bool, R: Resource>(&mut self, id: ObjectId, resource: R) {
        let cell = ResourceCell::new::<SEND, R>(resource);
        self.0.insert(id, cell);
    }

    pub fn get<R: Resource>(&self, id: ObjectId) -> Option<&R> {
        self.0.get(&id).and_then(|cell| Some(cell.get::<R>()))
    }

    pub fn get_mut<R: Resource>(&mut self, id: ObjectId) -> Option<&mut R> {
        self.0
            .get_mut(&id)
            .and_then(|cell| Some(cell.get_mut::<R>()))
    }

    pub fn remove(&mut self, id: ObjectId) {
        self.0.shift_remove(&id);
    }
}

pub struct Resources {
    send: ResourceStorage,
    non_send: ResourceStorage,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            send: ResourceStorage::new(),
            non_send: ResourceStorage::new(),
        }
    }

    pub fn add<const SEND: bool, R: Resource>(&mut self, id: ObjectId, resource: R) {
        self.storage_mut::<SEND>().add::<SEND, R>(id, resource);
    }

    pub fn get<const SEND: bool, R: Resource>(&self, id: ObjectId) -> Option<&R> {
        self.storage::<SEND>().get(id)
    }

    pub fn get_mut<const SEND: bool, R: Resource>(&mut self, id: ObjectId) -> Option<&mut R> {
        self.storage_mut::<SEND>().get_mut(id)
    }

    pub fn try_get<const SEND: bool, R: Resource>(&self, id: ObjectId) -> Option<&R> {
        self.storage::<SEND>().get(id)
    }

    pub fn try_get_mut<const SEND: bool, R: Resource>(&mut self, id: ObjectId) -> Option<&mut R> {
        self.storage_mut::<SEND>().get_mut(id)
    }

    pub fn contains<const SEND: bool>(&self, id: ObjectId) -> bool {
        self.storage::<SEND>().0.contains_key(&id)
    }

    pub fn remove<const SEND: bool>(&mut self, id: ObjectId) {
        self.storage_mut::<SEND>().remove(id);
    }

    fn storage<const SEND: bool>(&self) -> &ResourceStorage {
        if SEND { &self.send } else { &self.non_send }
    }

    fn storage_mut<const SEND: bool>(&mut self) -> &mut ResourceStorage {
        if SEND {
            &mut self.send
        } else {
            &mut self.non_send
        }
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
