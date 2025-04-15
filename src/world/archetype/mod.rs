use super::{Component, ComponentId, Components, Entity};
use crate::core::{Frame, bitset::FixedBitSet, storage::SparseIndex};
use std::{collections::HashMap, fmt::Debug};

pub mod table;

pub use table::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchetypeId(pub u32);

impl ArchetypeId {
    pub const EMPTY: Self = Self(0);
}

pub struct Archetype {
    id: ArchetypeId,
    table: Table,
    bitset: FixedBitSet,
}

impl Archetype {
    pub fn new(id: ArchetypeId, table: Table, bitset: FixedBitSet) -> Self {
        Self { id, table, bitset }
    }

    pub fn id(&self) -> ArchetypeId {
        self.id
    }

    pub fn table(&self) -> &Table {
        &self.table
    }

    pub fn contains(&self, components: &FixedBitSet) -> bool {
        self.bitset.is_superset(components)
    }

    pub fn has_component(&self, component: usize) -> bool {
        self.bitset[component]
    }

    pub fn has_component_id(&self, id: ComponentId) -> bool {
        self.table.has_component(id)
    }

    pub fn add_entity(&mut self, entity: Entity, row: Row) {
        self.table.add_entity(entity, row);
    }

    pub fn remove_entity(&mut self, entity: Entity) -> Option<Row> {
        self.table.remove_entity(entity)
    }

    pub fn modify_component(&mut self, entity: Entity, id: ComponentId, frame: Frame) {
        self.table.modify_component(entity, id, frame);
    }
}

pub struct Archetypes {
    archetypes: Vec<Archetype>,
    archetype_map: HashMap<Box<[ComponentId]>, ArchetypeId>,
    entity_map: HashMap<Entity, ArchetypeId>,
    components: Components,
    bitset: FixedBitSet,
}

impl Archetypes {
    pub fn new() -> Self {
        let archetypes = vec![Archetype::new(
            ArchetypeId::EMPTY,
            TableBuilder::new().build(),
            FixedBitSet::new(),
        )];

        let mut archetype_map: HashMap<Box<[ComponentId]>, ArchetypeId> = HashMap::new();
        archetype_map.insert(Box::new([]), ArchetypeId::EMPTY);

        Self {
            archetypes,
            archetype_map,
            entity_map: HashMap::new(),
            components: Components::new(),
            bitset: FixedBitSet::new(),
        }
    }

    pub fn register<C: Component>(&mut self) -> ComponentId {
        let id = self.components.register::<C>();
        self.bitset.grow(id.to_usize() + 1);
        id
    }

    pub fn archetypes(&self) -> &Vec<Archetype> {
        &self.archetypes
    }

    pub fn archetype(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(id.0 as usize)
    }

    pub fn entity_archetype(&self, entity: Entity) -> Option<ArchetypeId> {
        self.entity_map.get(&entity).copied()
    }

    pub fn components(&self) -> &Components {
        &self.components
    }

    pub fn components_mut(&mut self) -> &mut Components {
        &mut self.components
    }

    pub fn query(&self, query: &ArchetypeQuery) -> Vec<&Archetype> {
        let mut include = self.bitset.clone();
        for id in query.components.iter().copied() {
            include.set(id.to_usize(), true);
        }

        let mut exclude = self.bitset.clone();
        for id in query.exclude.iter() {
            exclude.set(id.to_usize(), true);
        }

        let mut archetypes = Vec::new();
        for archetype in &self.archetypes {
            if archetype.bitset.is_superset(&include) && exclude.is_disjoint(&archetype.bitset) {
                archetypes.push(archetype);
            }
        }

        archetypes
    }

    pub fn add_entity(&mut self, entity: Entity) -> ArchetypeId {
        match self.entity_map.get(&entity).copied() {
            Some(id) => id,
            None => {
                let archetype_id = ArchetypeId::EMPTY;
                self.entity_map.insert(entity, archetype_id);
                self.archetypes[archetype_id.0 as usize]
                    .table
                    .add_entity(entity, Row::new());
                archetype_id
            }
        }
    }

    pub fn remove_entity(&mut self, entity: Entity) -> Option<(ArchetypeId, Row)> {
        let id = self.entity_map.remove(&entity)?;
        let archetype = &mut self.archetypes[id.0 as usize];
        let row = archetype.remove_entity(entity)?;

        Some((id, row))
    }

    pub fn get_component<C: Component>(&self, entity: Entity) -> Option<&C> {
        let id = unsafe { self.components.get_id_unchecked::<C>() };
        let archetype_id = self.entity_map.get(&entity)?;
        let archetype = &self.archetypes[archetype_id.0 as usize];
        archetype.table.get_component(entity, id)
    }

    pub fn get_component_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        let id = unsafe { self.components.get_id_unchecked::<C>() };
        let archetype_id = self.entity_map.get(&entity)?;
        let archetype = &mut self.archetypes[archetype_id.0 as usize];
        archetype.table.get_component_mut(entity, id)
    }

    pub fn add_component<C: Component>(&mut self, frame: Frame, entity: Entity, component: C) {
        let id = unsafe { self.components.get_id_unchecked::<C>() };

        let (_, mut row) = match self.remove_entity(entity) {
            Some((id, row)) => (id, row),
            None => (ArchetypeId::EMPTY, Row::new()),
        };

        let mut component = TableCell::new(component);
        match row.contains(id) {
            true => component.modify(frame),
            false => component.add(frame),
        }

        row.insert_cell(id, component);

        self.add_entity_inner(entity, row);
    }

    pub fn add_components(&mut self, frame: Frame, entity: Entity, components: Row) {
        let (_, mut row) = match self.remove_entity(entity) {
            Some((id, row)) => (id, row),
            None => (ArchetypeId::EMPTY, Row::new()),
        };

        for (id, mut component) in components {
            match row.contains(id) {
                true => component.modify(frame),
                false => component.add(frame),
            }

            row.insert_cell(id, component);
        }

        self.add_entity_inner(entity, row);
    }

    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        let id = unsafe { self.components.get_id_unchecked::<C>() };

        let (_, mut row) = match self.remove_entity(entity) {
            Some(value) => value,
            None => return,
        };

        row.remove(id);

        self.add_entity_inner(entity, row);
    }

    pub fn remove_components(&mut self, entity: Entity, components: Vec<ComponentId>) {
        let (_, mut row) = match self.remove_entity(entity) {
            Some((id, row)) => (id, row),
            None => return,
        };

        let mut removed = Row::new();
        for id in components {
            if let Some(value) = row.remove(id) {
                removed.insert_cell(id, value);
            }
        }

        self.add_entity_inner(entity, row);
    }

    pub fn modify_component<C: Component>(&mut self, entity: Entity, frame: Frame) {
        let id = unsafe { self.components.get_id_unchecked::<C>() };

        let Some(archetype_id) = self.entity_map.get(&entity) else {
            return;
        };
        let archetype = &mut self.archetypes[archetype_id.0 as usize];
        archetype.modify_component(entity, id, frame);
    }

    #[inline]
    fn add_entity_inner(&mut self, entity: Entity, components: Row) -> ArchetypeId {
        let mut ids = components.ids().to_vec();
        ids.sort();

        let id = ids.into_boxed_slice();

        match self.archetype_map.get(&id).copied() {
            Some(id) => {
                let archetype = &mut self.archetypes[id.0 as usize];
                archetype.table.add_entity(entity, components);
                self.entity_map.insert(entity, id);

                id
            }
            None => {
                let mut bits = self.bitset.clone();
                id.iter().for_each(|id| bits.set(id.to_usize(), true));

                let archetype_id = ArchetypeId(self.archetypes.len() as u32);
                let archetype = Archetype::new(archetype_id, components.into_table(entity), bits);

                self.archetypes.push(archetype);
                self.entity_map.insert(entity, archetype_id);
                self.archetype_map.insert(id, archetype_id);
                archetype_id
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ArchetypeQuery {
    pub components: Vec<ComponentId>,
    pub exclude: Vec<ComponentId>,
}
