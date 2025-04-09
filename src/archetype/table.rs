use crate::core::{
    Component, ComponentId, Entity, Frame,
    blob::{Blob, BlobCell, Ptr},
    frame::ObjectTracker,
    storage::{ImmutableSparseSet, SparseIndex, SparseSet},
};
use indexmap::IndexSet;
use std::alloc::Layout;

pub struct TableCell {
    data: BlobCell,
    frame: ObjectTracker,
}

impl TableCell {
    pub fn new<T: Component>(value: T) -> Self {
        Self {
            data: BlobCell::new::<T>(value),
            frame: ObjectTracker::new(),
        }
    }

    pub fn with_frame<T: Component>(value: T, frame: Frame) -> Self {
        Self {
            data: BlobCell::new::<T>(value),
            frame: ObjectTracker {
                added: frame,
                modified: frame,
            },
        }
    }

    pub fn cell(&self) -> &BlobCell {
        &self.data
    }

    pub fn get<T: Component>(&self) -> &T {
        self.data.value::<T>()
    }

    pub fn get_mut<T: Component>(&mut self) -> &mut T {
        self.data.value_mut::<T>()
    }

    pub fn layout(&self) -> &Layout {
        self.data.layout()
    }

    pub fn drop(&self) -> Option<&fn(*mut u8)> {
        self.data.drop()
    }

    pub fn frame(&self) -> &ObjectTracker {
        &self.frame
    }

    pub fn add(&mut self, frame: Frame) {
        self.frame.added = frame;
    }

    pub fn modify(&mut self, frame: Frame) {
        self.frame.modified = frame;
    }
}

pub struct Column {
    data: Blob,
    frames: Vec<ObjectTracker>,
}

impl Column {
    pub fn new<T: Component>() -> Self {
        Self {
            data: Blob::new::<T>(1),
            frames: Vec::new(),
        }
    }

    pub fn with_layout(layout: Layout, drop: Option<fn(*mut u8)>) -> Self {
        Self {
            data: Blob::with_layout(layout, 1, drop),
            frames: Vec::new(),
        }
    }

    pub fn get<T: Component>(&self, index: usize) -> Option<&T> {
        self.data.get::<T>(index)
    }

    pub fn get_mut<T: Component>(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut::<T>(index)
    }

    pub unsafe fn get_ptr<T: Component>(&self) -> (Ptr<'_, T>, Ptr<'_, ObjectTracker>) {
        let components = unsafe { self.data.ptr::<T>(0) };
        let frames = self.frames.as_ptr() as *mut ObjectTracker;

        (components, Ptr::new(frames))
    }

    pub fn frames(&self) -> &[ObjectTracker] {
        &self.frames
    }

    pub fn push<T: Component>(&mut self, value: T) {
        self.data.push(value);
    }

    pub fn push_cell(&mut self, cell: TableCell) {
        self.data.push_cell(cell.data);
        self.frames.push(cell.frame);
    }

    pub fn remove(&mut self, index: usize) -> Option<TableCell> {
        let data = self.data.remove_cell_checked(index)?;
        let frame = self.frames.remove(index);
        Some(TableCell { data, frame })
    }

    pub fn swap_remove(&mut self, index: usize) -> Option<TableCell> {
        let data = self.data.swap_remove_cell_checked(index)?;
        let frame = self.frames.swap_remove(index);
        Some(TableCell { data, frame })
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }
}

impl From<TableCell> for Column {
    fn from(value: TableCell) -> Self {
        Self {
            data: Blob::from_cell(value.data),
            frames: vec![value.frame],
        }
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

    pub fn get_cell(&self, id: ComponentId) -> Option<&TableCell> {
        self.0.get(id)
    }

    pub fn insert<T: Component>(&mut self, id: ComponentId, value: T) -> Option<TableCell> {
        self.0.insert(id, TableCell::new(value))
    }

    pub fn insert_cell(&mut self, id: ComponentId, cell: TableCell) -> Option<TableCell> {
        self.0.insert(id, cell)
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

    pub fn ids(&self) -> &[ComponentId] {
        self.0.indices()
    }

    pub fn components(&self) -> &[TableCell] {
        self.0.values()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ComponentId, &TableCell)> {
        self.0.iter()
    }

    pub fn contains(&self, id: ComponentId) -> bool {
        self.0.contains(id)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RowIndex(pub u32);
impl std::ops::Deref for RowIndex {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SparseIndex for RowIndex {
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

    pub fn get_entity_row(&self, entity: Entity) -> Option<RowIndex> {
        let index = self.entities.get_index_of(&entity)?;
        Some(RowIndex(index as u32))
    }

    pub fn entities(&self) -> indexmap::set::Iter<'_, Entity> {
        self.entities.iter()
    }

    pub fn get_column(&self, component: ComponentId) -> Option<&Column> {
        self.columns.get(component)
    }

    pub fn get_component<C: Component>(
        &self,
        entity: Entity,
        component: ComponentId,
    ) -> Option<&C> {
        let index = self.entities.get_index_of(&entity)?;
        let column = self.columns.get(component)?;
        column.get::<C>(index)
    }

    pub fn get_component_mut<C: Component>(
        &mut self,
        entity: Entity,
        component: ComponentId,
    ) -> Option<&mut C> {
        let index = self.entities.get_index_of(&entity)?;
        let column = self.columns.get_mut(component)?;
        column.get_mut::<C>(index)
    }

    pub fn has_component(&self, id: ComponentId) -> bool {
        self.columns.contains(id)
    }
}
