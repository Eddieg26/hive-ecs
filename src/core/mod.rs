use std::{alloc::Layout, any::TypeId, collections::HashMap, fmt::Debug};

pub mod bitset;
pub mod blob;
pub mod event;
pub mod frame;
pub mod resource;
pub mod storage;
pub mod table;

pub use event::*;
pub use frame::*;
pub use resource::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    id: u32,
    generation: u32,
}

impl Entity {
    pub fn new(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }

    pub fn root(id: u32) -> Self {
        Self { id, generation: 0 }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Entity {{ id: {}, generation: {} }}",
            self.id, self.generation
        )
    }
}

pub struct Entities {
    current: u32,
    free: Vec<u32>,
    generations: HashMap<u32, u32>,
}

impl Entities {
    pub fn new() -> Self {
        Self {
            current: 0,
            free: vec![],
            generations: HashMap::new(),
        }
    }

    pub fn spawn(&mut self) -> Entity {
        if let Some(id) = self.free.pop() {
            let generation = self.generations.entry(id).or_default();
            *generation += 1;

            Entity::new(id, *generation)
        } else {
            let id = self.current;
            let generation = 1;
            self.generations.insert(id, generation);
            self.current += 1;

            Entity::new(id, generation)
        }
    }

    pub fn despawn(&mut self, entity: Entity) {
        self.free.push(entity.id);
    }

    pub fn clear(&mut self) {
        self.current = 0;
        self.free.clear();
        self.generations.clear();
    }
}

pub trait Component: Send + Sync + 'static {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(pub(crate) u32);

pub struct ComponentMeta {
    id: ComponentId,
    name: &'static str,
    layout: Layout,
}

impl ComponentMeta {
    pub fn new<C: Component>(id: ComponentId) -> Self {
        let name = std::any::type_name::<C>();

        Self {
            id,
            name: if let Some(index) = name.rfind(":") {
                &name[index..]
            } else {
                name
            },
            layout: Layout::new::<C>(),
        }
    }

    pub fn id(&self) -> ComponentId {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }
}

pub struct Components {
    components: Vec<ComponentMeta>,
    map: HashMap<TypeId, ComponentId>,
}

impl Components {
    pub fn new() -> Self {
        Self {
            components: vec![],
            map: HashMap::new(),
        }
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        let ty = TypeId::of::<C>();
        match self.map.get(&ty) {
            Some(id) => *id,
            None => {
                let id = ComponentId(self.components.len() as u32);
                let meta = ComponentMeta::new::<C>(id);

                self.components.push(meta);
                self.map.insert(TypeId::of::<C>(), id);

                id
            }
        }
    }

    pub fn get<C: Component>(&self) -> Option<&ComponentMeta> {
        self.map.get(&TypeId::of::<C>()).and_then(|id| {
            self.components
                .get(id.0 as usize)
                .filter(|meta| meta.id == *id)
        })
    }

    pub fn get_id<C: Component>(&self) -> Option<ComponentId> {
        self.map.get(&TypeId::of::<C>()).copied()
    }

    pub unsafe fn get_id_unchecked<C: Component>(&self) -> ComponentId {
        self.map
            .get(&TypeId::of::<C>())
            .copied()
            .unwrap_or_else(|| panic!("Component not registered: {}", std::any::type_name::<C>()))
    }
}
