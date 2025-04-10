use super::World;
use std::marker::PhantomData;

#[derive(Clone, Copy)]
pub struct WorldCell<'w>(*mut World, PhantomData<&'w mut World>);
impl<'w> WorldCell<'w> {
    pub(crate) fn new(world: &World) -> Self {
        WorldCell(std::ptr::from_ref(world).cast_mut(), PhantomData)
    }

    pub fn new_mut(world: &mut World) -> Self {
        WorldCell(std::ptr::from_mut(world), PhantomData)
    }

    pub unsafe fn get(&self) -> &'w World {
        unsafe { &*self.0 }
    }

    pub unsafe fn get_mut(&mut self) -> &'w mut World {
        unsafe { &mut *self.0 }
    }
}
