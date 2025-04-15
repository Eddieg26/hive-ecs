use crate::core::{Frame, ObjectStatus, blob::Ptr, storage::SparseIndex};
use crate::system::Access;
use crate::world::{
    Component, ComponentId, Components, Entity, World,
    archetype::{
        Archetype, ArchetypeQuery,
        table::{Column, RowIndex},
    },
    cell::WorldCell,
};

use super::SystemAccess;
use super::arg::SystemArg;

pub trait BaseQuery {
    type Item<'w>;
    type State<'w>;

    /// Data used to construct the state of the query.
    /// This is used to create the query state when the query is first created.
    type Data: Send + Sync + Sized;

    fn init(components: &Components, query: &mut ArchetypeQuery) -> Self::Data;

    fn state<'w>(
        data: &Self::Data,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w>;

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w>;

    fn access(_: &Self::Data) -> Vec<SystemAccess> {
        vec![]
    }
}

pub trait BaseFilter: for<'w> BaseQuery<Item<'w> = bool> {}

impl<Q: for<'w> BaseQuery<Item<'w> = bool>> BaseFilter for Q {}

impl BaseQuery for () {
    type Item<'w> = bool;

    type State<'w> = ();

    type Data = ();

    fn init(_: &Components, _: &mut ArchetypeQuery) -> Self::Data {
        ()
    }

    fn state<'w>(_: &Self::Data, _: &'w Archetype, _: Frame, _: Frame) -> Self::State<'w> {
        ()
    }

    fn get<'w>(_: &mut Self::State<'w>, _: Entity, _: RowIndex) -> Self::Item<'w> {
        true
    }
}

pub struct Not<C: Component>(std::marker::PhantomData<C>);
impl<C: Component> BaseQuery for Not<C> {
    type Item<'w> = bool;

    type State<'w> = ();

    type Data = ();

    fn init(components: &Components, state: &mut ArchetypeQuery) -> Self::Data {
        let id = components.get_id::<C>().expect(&format!(
            "Component not registered: {}",
            std::any::type_name::<C>()
        ));

        state.exclude.push(id);

        ()
    }

    fn state<'w>(_: &Self::Data, _: &'w Archetype, _: Frame, _: Frame) -> Self::State<'w> {
        ()
    }

    fn get<'w>(_: &mut Self::State<'w>, _: Entity, _: RowIndex) -> Self::Item<'w> {
        true
    }
}

pub struct With<C: Component>(std::marker::PhantomData<C>);
impl<C: Component> BaseQuery for With<C> {
    type Item<'w> = bool;
    type State<'w> = ();
    type Data = ();

    fn init(components: &Components, state: &mut ArchetypeQuery) -> Self::Data {
        let id = components.get_id::<C>().expect(&format!(
            "Component not registered: {}",
            std::any::type_name::<C>()
        ));

        state.components.push(id);

        ()
    }

    fn state<'w>(_: &Self::Data, _: &'w Archetype, _: Frame, _: Frame) -> Self::State<'w> {
        ()
    }

    fn get<'w>(_: &mut Self::State<'w>, _: Entity, _: RowIndex) -> Self::Item<'w> {
        todo!()
    }
}

pub struct Added<T: 'static>(std::marker::PhantomData<T>);
pub struct AddedComponent<'w, C: Component> {
    reader: Option<ReadQuery<'w, C>>,
    current_frame: Frame,
    system_frame: Frame,
}

impl<C: Component> BaseQuery for Added<C> {
    type Item<'w> = bool;
    type State<'w> = AddedComponent<'w, C>;
    type Data = ComponentId;

    fn init(components: &Components, _: &mut ArchetypeQuery) -> Self::Data {
        let id = components.get_id::<C>().expect(&format!(
            "Component not registered: {}",
            std::any::type_name::<C>()
        ));

        id
    }

    fn state<'w>(
        data: &Self::Data,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        let components = archetype.table().get_column(*data);
        AddedComponent {
            reader: components.map(|components| ReadQuery::from(components)),
            current_frame,
            system_frame,
        }
    }

    fn get<'w>(state: &mut Self::State<'w>, _: Entity, row: RowIndex) -> Self::Item<'w> {
        match state.reader.as_ref() {
            Some(reader) => reader.components.frames()[row.to_usize()]
                .added
                .is_newer(state.current_frame, state.system_frame),
            None => false,
        }
    }
}

pub struct Modified<T: 'static>(std::marker::PhantomData<T>);
pub struct ModifiedComponent<'w, C: Component> {
    reader: Option<ReadQuery<'w, C>>,
    current_frame: Frame,
    system_frame: Frame,
}

impl<C: Component> BaseQuery for Modified<C> {
    type Item<'w> = bool;
    type State<'w> = ModifiedComponent<'w, C>;
    type Data = ComponentId;

    fn init(components: &Components, _: &mut ArchetypeQuery) -> Self::Data {
        let id = components.get_id::<C>().expect(&format!(
            "Component not registered: {}",
            std::any::type_name::<C>()
        ));

        id
    }

    fn state<'w>(
        data: &Self::Data,
        archetype: &'w Archetype,
        current_frame: Frame,
        system_frame: Frame,
    ) -> Self::State<'w> {
        let components = archetype.table().get_column(*data);
        ModifiedComponent {
            reader: components.map(|components| ReadQuery::from(components)),
            current_frame,
            system_frame,
        }
    }

    fn get<'w>(state: &mut Self::State<'w>, _: Entity, row: RowIndex) -> Self::Item<'w> {
        match state.reader.as_ref() {
            Some(reader) => {
                let modified = reader.components.frames()[row.to_usize()].modified;
                modified.is_newer(state.current_frame, state.system_frame)
            }
            None => false,
        }
    }
}

pub struct ReadQuery<'a, C: Component> {
    components: &'a Column,
    _marker: std::marker::PhantomData<C>,
}

impl<'a, C: Component> From<&'a Column> for ReadQuery<'a, C> {
    fn from(components: &'a Column) -> Self {
        Self {
            components,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<C: Component> BaseQuery for &C {
    type Item<'w> = &'w C;

    type State<'w> = ReadQuery<'w, C>;

    type Data = ComponentId;

    fn init(components: &Components, query: &mut ArchetypeQuery) -> Self::Data {
        let id = components.get_id::<C>().expect(&format!(
            "Component not registered: {}",
            std::any::type_name::<C>()
        ));

        query.components.push(id);

        id
    }

    fn state<'w>(
        data: &Self::Data,
        archetype: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        let components = archetype.table().get_column(*data).expect(&format!(
            "Component not found in archetype: {}",
            std::any::type_name::<C>()
        ));

        ReadQuery::from(components)
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        state
            .components
            .get(row.to_usize())
            .expect(&format!("Component not found for entity: {:?}", entity))
    }

    fn access(data: &Self::Data) -> Vec<SystemAccess> {
        vec![SystemAccess::Component {
            id: *data,
            access: Access::Read,
        }]
    }
}

pub struct WriteQuery<'a, C: Component> {
    components: Ptr<'a, C>,
    frames: Ptr<'a, ObjectStatus>,
    current_frame: Frame,
    _marker: std::marker::PhantomData<C>,
}

impl<'a, C: Component> WriteQuery<'a, C> {
    pub fn new(
        components: Ptr<'a, C>,
        frames: Ptr<'a, ObjectStatus>,
        current_frame: Frame,
    ) -> Self {
        Self {
            components,
            frames,
            current_frame,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<C: Component> BaseQuery for &mut C {
    type Item<'w> = &'w mut C;

    type State<'w> = WriteQuery<'w, C>;

    type Data = ComponentId;

    fn init(components: &Components, query: &mut ArchetypeQuery) -> Self::Data {
        <&C as BaseQuery>::init(components, query)
    }

    fn state<'w>(
        data: &Self::Data,
        archetype: &'w Archetype,
        current_frame: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        let (components, frames) = unsafe {
            archetype
                .table()
                .get_column(*data)
                .expect(&format!(
                    "Component not found in archetype: {}",
                    std::any::type_name::<C>()
                ))
                .get_ptr()
        };

        WriteQuery::new(components, frames, current_frame)
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        let component = state
            .components
            .get_mut(row.to_usize())
            .expect(&format!("Component not found for entity: {:?}", entity));

        state.frames.get_mut(row.0 as usize).unwrap().modified = state.current_frame;

        component
    }

    fn access(data: &Self::Data) -> Vec<SystemAccess> {
        vec![SystemAccess::Component {
            id: *data,
            access: Access::Write,
        }]
    }
}

impl<C: Component> BaseQuery for Option<&C> {
    type Item<'w> = Option<&'w C>;

    type State<'w> = Option<ReadQuery<'w, C>>;

    type Data = ComponentId;

    fn init(components: &Components, _: &mut ArchetypeQuery) -> Self::Data {
        let id = components.get_id::<C>().expect(&format!(
            "Component not registered: {}",
            std::any::type_name::<C>()
        ));

        id
    }

    fn state<'w>(
        data: &Self::Data,
        archetype: &'w Archetype,
        _: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        archetype
            .table()
            .get_column(*data)
            .map(|column| ReadQuery::from(column))
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        match state {
            Some(state) => Some(<&C as BaseQuery>::get(state, entity, row)),
            None => None,
        }
    }

    fn access(data: &Self::Data) -> Vec<SystemAccess> {
        <&C as BaseQuery>::access(data)
    }
}

impl<C: Component> BaseQuery for Option<&mut C> {
    type Item<'w> = Option<&'w mut C>;

    type State<'w> = Option<WriteQuery<'w, C>>;

    type Data = ComponentId;

    fn init(components: &Components, _: &mut ArchetypeQuery) -> Self::Data {
        let id = components.get_id::<C>().expect(&format!(
            "Component not registered: {}",
            std::any::type_name::<C>()
        ));

        id
    }

    fn state<'w>(
        data: &Self::Data,
        archetype: &'w Archetype,
        current_frame: Frame,
        _: Frame,
    ) -> Self::State<'w> {
        archetype.table().get_column(*data).map(|column| {
            let (components, frames) = unsafe { column.get_ptr() };
            WriteQuery::new(components, frames, current_frame)
        })
    }

    fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
        match state {
            Some(state) => Some(<&mut C as BaseQuery>::get(state, entity, row)),
            None => None,
        }
    }

    fn access(data: &Self::Data) -> Vec<SystemAccess> {
        <&mut C as BaseQuery>::access(data)
    }
}

impl BaseQuery for Entity {
    type Item<'w> = Entity;

    type State<'w> = ();

    type Data = ();

    fn init(_: &Components, _: &mut ArchetypeQuery) -> Self::Data {
        ()
    }

    fn state<'w>(_: &Self::Data, _: &'w Archetype, _: Frame, _: Frame) -> Self::State<'w> {
        ()
    }

    fn get<'w>(_: &mut Self::State<'w>, entity: Entity, _: RowIndex) -> Self::Item<'w> {
        entity
    }
}

pub struct QueryState<Q: BaseQuery, F: BaseFilter = ()> {
    pub(crate) query: ArchetypeQuery,
    pub(crate) data: Q::Data,
    pub(crate) filter_data: F::Data,
}

impl<Q: BaseQuery, F: BaseFilter> QueryState<Q, F> {
    pub fn new(world: &World) -> Self {
        let mut query = ArchetypeQuery::default();
        let data = Q::init(world.components(), &mut query);
        let filter_data = F::init(world.components(), &mut query);

        QueryState {
            query,
            data,
            filter_data,
        }
    }
}

pub struct Query<'w, 's, Q: BaseQuery, F: BaseFilter = ()> {
    world: WorldCell<'w>,
    state: &'s QueryState<Q, F>,
    current_frame: Frame,
    system_frame: Frame,
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> Query<'w, 's, Q, F> {
    pub fn new(world: &'w World, state: &'s QueryState<Q, F>) -> Self {
        Self {
            world: unsafe { WorldCell::new(world) },
            current_frame: world.frame(),
            system_frame: world.frame().previous(),
            state,
        }
    }

    pub fn with_frame(world: &'w World, state: &'s QueryState<Q, F>, frame: Frame) -> Self {
        Self {
            world: unsafe { WorldCell::new(world) },
            current_frame: world.frame(),
            system_frame: frame,
            state,
        }
    }

    pub fn iter(&'w self) -> QueryIter<'w, 's, Q, F> {
        QueryIter::new(&self)
    }
}

unsafe impl<Q: BaseQuery + 'static, F: BaseFilter + 'static> SystemArg for Query<'_, '_, Q, F> {
    type Item<'world, 'state> = Query<'world, 'state, Q, F>;

    type State = QueryState<Q, F>;

    fn init(world: &mut World) -> Self::State {
        QueryState::new(world)
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        world: WorldCell<'world>,
        system: &super::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { Query::with_frame(world.get(), state, system.frame) }
    }

    fn access(state: &Self::State) -> Vec<super::SystemAccess> {
        Q::access(&state.data)
    }
}

pub struct QueryIter<'w, 's, Q: BaseQuery, F: BaseFilter = ()> {
    query: &'w Query<'w, 's, Q, F>,
    archetypes: Vec<&'w Archetype>,
    state: Option<Q::State<'w>>,
    filter: Option<F::State<'w>>,
    entities: Option<indexmap::set::Iter<'w, Entity>>,
    archetype: usize,
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> QueryIter<'w, 's, Q, F> {
    pub fn new(query: &'w Query<'w, 's, Q, F>) -> Self {
        let world = unsafe { query.world.get() };
        let archetypes = world.archetypes().query(&query.state.query);

        let (state, filter_state, entities) = archetypes
            .get(0)
            .map(|archetype| {
                let state = Q::state(
                    &query.state.data,
                    archetype,
                    query.current_frame,
                    query.system_frame,
                );
                let filter_state = F::state(
                    &query.state.filter_data,
                    archetype,
                    query.current_frame,
                    query.system_frame,
                );

                let entities = archetype.table().entities();

                (Some(state), Some(filter_state), Some(entities))
            })
            .unwrap_or((None, None, None));

        Self {
            query,
            archetypes,
            state,
            filter: filter_state,
            entities,
            archetype: 0,
        }
    }
}

impl<'w, 's, Q: BaseQuery, F: BaseFilter> Iterator for QueryIter<'w, 's, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.archetype >= self.archetypes.len() {
            None
        } else if let Some(entity) = self
            .entities
            .as_mut()
            .and_then(|entities| entities.next())
            .copied()
        {
            let row = self.archetypes[self.archetype]
                .table()
                .get_entity_row(entity)
                .unwrap();

            let state = self.state.as_mut()?;
            let filter = match &mut self.filter {
                Some(state) => F::get(state, entity, row),
                None => true,
            };

            filter.then_some(Q::get(state, entity, row))
        } else {
            self.archetype += 1;
            self.entities = self.archetypes.get(self.archetype).map(|archetype| {
                self.state = Some(Q::state(
                    &self.query.state.data,
                    archetype,
                    self.query.current_frame,
                    self.query.system_frame,
                ));
                self.filter = Some(F::state(
                    &self.query.state.filter_data,
                    archetype,
                    self.query.current_frame,
                    self.query.system_frame,
                ));
                archetype.table().entities()
            });

            self.next()
        }
    }
}

#[macro_export]
macro_rules! impl_base_query_for_tuples {
    ($(($($name:ident),*)),*)  => {
        $(
            #[allow(non_snake_case)]
            impl<$($name: BaseQuery),+> BaseQuery for ($($name),+) {
                type Item<'w> = ($($name::Item<'w>), +);

                type State<'w> = ($($name::State<'w>), +);

                type Data = ($($name::Data), +);

                fn init(components: &Components, query: &mut ArchetypeQuery) -> Self::Data {
                    ($($name::init(components, query),)*)
                }

                fn state<'w>(data: &Self::Data, archetype: &'w Archetype, current_frame: Frame, system_frame: Frame) -> Self::State<'w> {
                    let ($($name,)*) = data;
                    ($($name::state($name, archetype, current_frame, system_frame),)*)
                }

                fn get<'w>(state: &mut Self::State<'w>, entity: Entity, row: RowIndex) -> Self::Item<'w> {
                    let ($($name,)*) = state;

                    ($(
                        $name::get($name, entity, row),
                    )*)
                }

                fn access(data: &Self::Data) -> Vec<SystemAccess> {
                    let ($($name,)*) = data;
                    let mut access = vec![];
                    $(
                        access.extend($name::access($name));
                    )*
                    access
                }
            }
        )+
    };
}

impl_base_query_for_tuples!((A, B));
impl_base_query_for_tuples!((A, B, C));
impl_base_query_for_tuples!((A, B, C, D));
impl_base_query_for_tuples!((A, B, C, D, E));
impl_base_query_for_tuples!((A, B, C, D, E, F));
impl_base_query_for_tuples!((A, B, C, D, E, F, G));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P));
impl_base_query_for_tuples!((A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q));

#[cfg(test)]
mod tests {

    use crate::{
        core::bitset::FixedBitSet,
        world::archetype::{
            ArchetypeId,
            table::{Row, TableCell},
        },
    };

    use super::*;

    impl Component for i32 {}

    #[test]
    fn test_modified_filter() {
        let mut components = Components::new();
        let mut archetype_query = ArchetypeQuery::default();

        // Register a component
        let component_id = components.register::<i32>();

        // Initialize the Modified filter
        let modified_filter = Modified::<i32>::init(&components, &mut archetype_query);

        let system_frame = Frame(0);
        let current_frame = Frame(1);

        // Create a mock archetype with a table for the component
        let mut row = Row::new();
        row.insert_cell(component_id, TableCell::with_frame(10, current_frame));
        let archetype = Archetype::new(
            ArchetypeId(0),
            row.into_table(Entity::root(0)),
            FixedBitSet::new(),
        );

        // Check if the filter detects the modification
        let mut state =
            Modified::<i32>::state(&modified_filter, &archetype, current_frame, system_frame);
        let row = RowIndex(0);
        assert!(Modified::<i32>::get(&mut state, Entity::root(0), row));
    }
}
