use crate::core::{
    Component, Entity,
    blob::{Blob, BlobCell, Ptr},
    registry::ObjectId,
    storage::{ImmutableSparseSet, SparseIndex, SparseSet},
};
use indexmap::IndexSet;
use std::alloc::Layout;

pub type ComponentId = ObjectId;

pub struct TableCell(BlobCell);
impl TableCell {
    pub fn new<T: Component>(value: T) -> Self {
        Self(BlobCell::new::<T>(value))
    }

    pub fn get<T: Component>(&self) -> &T {
        self.0.value::<T>()
    }

    pub fn get_mut<T: Component>(&mut self) -> &mut T {
        self.0.value_mut::<T>()
    }

    pub fn layout(&self) -> &Layout {
        self.0.layout()
    }

    pub fn drop(&self) -> Option<&fn(*mut u8)> {
        self.0.drop()
    }
}

pub struct Column(Blob);

impl Column {
    pub fn new<T: Component>() -> Self {
        Self(Blob::new::<T>(1))
    }

    pub fn with_layout(layout: Layout, drop: Option<fn(*mut u8)>) -> Self {
        Self(Blob::with_layout(layout, 1, drop))
    }

    pub fn get<T: Component>(&self, index: usize) -> Option<&T> {
        self.0.get::<T>(index)
    }

    pub fn get_mut<T: Component>(&mut self, index: usize) -> Option<&mut T> {
        self.0.get_mut::<T>(index)
    }

    pub fn get_ptr<T: Component>(&self, index: usize) -> Ptr<'_, T> {
        unsafe { self.0.ptr::<T>(index) }
    }

    pub fn push<T: Component>(&mut self, value: T) {
        self.0.push(value);
    }

    pub fn push_cell(&mut self, cell: TableCell) {
        self.0.push_cell(cell.0);
    }

    pub fn remove(&mut self, index: usize) -> Option<TableCell> {
        self.0.remove_cell_checked(index).map(TableCell)
    }

    pub fn swap_remove(&mut self, index: usize) -> Option<TableCell> {
        self.0.swap_remove_cell_checked(index).map(TableCell)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl From<TableCell> for Column {
    fn from(value: TableCell) -> Self {
        Self(value.0.into())
    }
}

pub struct Row(SparseSet<ComponentId, TableCell>);

impl Row {
    pub fn new() -> Self {
        Self(SparseSet::new())
    }

    pub fn get<T: Component>(&self, id: ComponentId) -> Option<&T> {
        self.0.get(id).map(|cell| cell.get::<T>())
    }

    pub fn get_mut<T: Component>(&mut self, id: ComponentId) -> Option<&mut T> {
        self.0.get_mut(id).map(|cell| cell.get_mut::<T>())
    }

    pub fn insert<T: Component>(&mut self, id: ComponentId, value: T) {
        self.0.insert(id, TableCell::new(value));
    }

    pub fn insert_cell(&mut self, id: ComponentId, cell: TableCell) {
        self.0.insert(id, cell);
    }

    pub fn remove(&mut self, id: ComponentId) -> Option<TableCell> {
        self.0.remove(id)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ComponentId, &TableCell)> {
        self.0.iter()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn into_table(mut self, entity: Entity) -> Table {
        let columns = self
            .0
            .drain()
            .map(|(id, cell)| (id, Column::from(cell)))
            .collect::<SparseSet<ComponentId, Column>>();

        let mut entities = IndexSet::new();
        entities.insert(entity);

        Table {
            entities,
            columns: columns.into(),
        }
    }
}

impl Iterator for Row {
    type Item = (ComponentId, TableCell);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.drain().next().map(|(id, cell)| (id, cell))
    }
}

impl SparseIndex for ComponentId {
    fn to_usize(self) -> usize {
        self.0 as usize
    }

    fn from_usize(index: usize) -> Self {
        Self(index as u32)
    }
}

pub struct TableBuilder {
    columns: SparseSet<ComponentId, Column>,
}

impl TableBuilder {
    pub fn new() -> Self {
        Self {
            columns: SparseSet::new(),
        }
    }

    pub fn with_column<T: Component>(mut self, component_id: ComponentId) -> Self {
        self.add_column::<T>(component_id);
        self
    }

    pub fn add_column<T: Component>(&mut self, component_id: ComponentId) -> &mut Self {
        self.columns.insert(component_id, Column::new::<T>());
        self
    }

    pub fn build(self) -> Table {
        Table {
            entities: IndexSet::new(),
            columns: self.columns.into(),
        }
    }
}

pub struct Table {
    entities: IndexSet<Entity>,
    columns: ImmutableSparseSet<ComponentId, Column>,
}

impl Table {
    pub fn add_entity(&mut self, entity: Entity, mut row: Row) {
        self.entities.insert(entity);

        self.columns.iter_mut().for_each(|(id, column)| {
            if let Some(cell) = row.remove(*id) {
                column.push_cell(cell);
            } else {
                panic!("Row does not contain all columns for entity: {:?}", entity);
            }
        });
    }

    pub fn remove_entity(&mut self, entity: Entity) -> Option<Row> {
        let index = self.entities.get_index_of(&entity)?;
        self.entities.swap_remove_index(index);

        let mut row = Row::new();
        self.columns.iter_mut().for_each(|(id, column)| {
            if let Some(cell) = column.swap_remove(index) {
                row.insert_cell(*id, cell);
            }
        });
        Some(row)
    }

    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter().copied()
    }

    pub fn get<T: Component>(&self, entity: Entity, id: ComponentId) -> Option<Ptr<'_, T>> {
        let index = self.entities.get_index_of(&entity)?;
        self.columns
            .get(id)
            .and_then(|column| Some(column.get_ptr::<T>(index)))
    }
}
