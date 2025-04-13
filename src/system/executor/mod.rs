use super::SystemConfig;
use crate::world::{World, WorldCell};

pub mod parallel;

pub use parallel::*;

pub trait SystemExecutor: 'static {
    fn execute(&self, world: WorldCell);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Sequential,
    Parallel,
}

impl RunMode {
    pub fn create_executor(
        &self,
        world: &mut World,
        configs: Vec<SystemConfig>,
    ) -> Box<dyn SystemExecutor> {
        match self {
            RunMode::Sequential => todo!(),
            RunMode::Parallel => Box::new(ParallelExecutor::new(world, configs)),
        }
    }
}
