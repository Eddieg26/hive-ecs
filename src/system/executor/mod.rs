use super::{SystemConfig, SystemNode};
use crate::world::{World, WorldCell};

pub mod parallel;
pub mod sequential;

use fixedbitset::FixedBitSet;
pub use parallel::*;
pub use sequential::*;

pub trait SystemExecutor: 'static {
    fn execute(&self, world: WorldCell);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Sequential,
    Parallel,
}

impl RunMode {
    pub fn create_executor(&self, info: GraphInfo) -> Box<dyn SystemExecutor> {
        match self {
            RunMode::Sequential => Box::new(SequentialExecutor::new(info)),
            RunMode::Parallel => Box::new(ParallelExecutor::new(info)),
        }
    }
}

pub struct GraphInfo {
    pub nodes: Vec<SystemNode>,
    pub dependents: Vec<FixedBitSet>,
    pub dependencies: Vec<usize>,
}

impl GraphInfo {
    pub fn new(world: &mut World, configs: Vec<SystemConfig>) -> Self {
        let mut nodes = vec![];
        for config in configs {
            let node = config.into_system_node(world);
            nodes.push(node);
        }

        let mut dependents = vec![FixedBitSet::with_capacity(nodes.len()); nodes.len()];
        let mut dependencies = vec![0usize; nodes.len()];
        for (index, node) in nodes.iter().rev().enumerate() {
            for (dep_index, dep_node) in nodes.iter().take(index).enumerate() {
                if node.has_dependency(dep_node) {
                    dependents[dep_index].set(index, true);
                    dependencies[index] += 1;
                }
            }
        }

        Self {
            nodes,
            dependents,
            dependencies,
        }
    }
}
