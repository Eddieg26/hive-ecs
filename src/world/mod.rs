use crate::{
    core::{
        Component, ComponentId, Components, Entities, Entity, Frame, NonSend, NonSendMut, Res,
        ResMut, Resource, ResourceId, Resources, table::Row,
    },
    world::archetype::{ArchetypeId, Archetypes},
};
use std::fmt::Debug;

pub mod archetype;
pub mod cell;
pub mod query;
pub mod system;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldId(u32);
impl WorldId {
    fn new() -> Self {
        static mut ID: u32 = 0;
        unsafe {
            let id = ID;
            ID += 1;
            WorldId(id)
        }
    }
}

pub struct World {
    id: WorldId,
    archetypes: Archetypes,
    resources: Resources,
    entities: Entities,
    frame: Frame,
}

impl World {
    pub fn new() -> Self {
        World {
            id: WorldId::new(),
            archetypes: Archetypes::new(),
            resources: Resources::new(),
            entities: Entities::new(),
            frame: Frame(1),
        }
    }

    pub fn id(&self) -> WorldId {
        self.id
    }

    pub fn components(&self) -> &Components {
        self.archetypes.components()
    }

    pub fn components_mut(&mut self) -> &mut Components {
        self.archetypes.components_mut()
    }

    pub fn archetypes(&self) -> &Archetypes {
        &self.archetypes
    }

    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    pub fn entities(&self) -> &Entities {
        &self.entities
    }

    pub fn frame(&self) -> Frame {
        self.frame
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        self.archetypes.register::<C>()
    }

    pub fn register_resource<R: Resource + Send>(&mut self) -> ResourceId {
        self.resources.register::<true, R>()
    }

    pub fn register_non_send_resource<R: Resource>(&mut self) -> ResourceId {
        self.resources.register::<false, R>()
    }

    pub fn add_resource<R: Resource + Send>(&mut self, resource: R) {
        self.resources.add::<true, R>(resource);
    }

    pub fn add_non_send_resource<R: Resource>(&mut self, resource: R) {
        self.resources.add::<false, R>(resource);
    }

    pub fn resource<R: Resource>(&self) -> &R {
        self.resources.get::<R>().expect(&format!(
            "Resource not found: {}",
            std::any::type_name::<R>()
        ))
    }

    pub fn resource_mut<R: Resource>(&mut self) -> &mut R {
        self.resources.get_mut::<R>().expect(&format!(
            "Resource not found: {}",
            std::any::type_name::<R>()
        ))
    }

    pub fn res<R: Resource + Send>(&self) -> Res<R> {
        let resource = self.resource::<R>();
        Res::new(resource)
    }

    pub fn res_mut<R: Resource + Send>(&mut self) -> ResMut<R> {
        let resource = self.resource_mut::<R>();
        ResMut::new(resource)
    }

    pub fn non_send_res<R: Resource>(&self) -> NonSend<R> {
        let resource = self.resource::<R>();
        NonSend::new(resource)
    }

    pub fn non_send_res_mut<R: Resource>(&mut self) -> NonSendMut<R> {
        let resource = self.resource_mut::<R>();
        NonSendMut::new(resource)
    }

    pub fn try_resource<R: Resource>(&self) -> Option<&R> {
        self.resources.get::<R>()
    }

    pub fn try_resource_mut<R: Resource>(&mut self) -> Option<&mut R> {
        self.resources.get_mut::<R>()
    }

    pub fn try_res<R: Resource + Send>(&self) -> Option<Res<R>> {
        self.resources.get::<R>().map(Res::new)
    }

    pub fn try_res_mut<R: Resource + Send>(&mut self) -> Option<ResMut<R>> {
        self.resources.get_mut::<R>().map(ResMut::new)
    }

    pub fn try_non_send_res<R: Resource>(&self) -> Option<NonSend<R>> {
        self.resources.get::<R>().map(NonSend::new)
    }

    pub fn try_non_send_resource_mut<R: Resource>(&mut self) -> Option<NonSendMut<R>> {
        self.resources.get_mut::<R>().map(NonSendMut::new)
    }

    pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
        self.resources.remove::<R>()
    }
}

impl World {
    pub fn spawn(&mut self) -> Entity {
        let entity = self.entities.spawn();
        self.archetypes.add_entity(entity);
        entity
    }

    pub fn despawn(&mut self, entity: Entity) -> Option<(ArchetypeId, Row)> {
        self.entities.despawn(entity);
        self.archetypes.remove_entity(entity)
    }

    pub fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        self.archetypes.get_component::<C>(entity)
    }

    pub fn get_component_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        self.archetypes.get_component_mut::<C>(entity)
    }

    pub fn add_component<C: Component>(&mut self, entity: Entity, component: C) {
        self.archetypes.add_component(self.frame, entity, component);
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        self.archetypes.remove_component::<C>(entity);
    }

    pub fn add_components(&mut self, entity: Entity, components: Row) {
        self.archetypes
            .add_components(self.frame, entity, components);
    }

    pub fn remove_components(&mut self, entity: Entity, components: Vec<ComponentId>) {
        self.archetypes.remove_components(entity, components);
    }
}
