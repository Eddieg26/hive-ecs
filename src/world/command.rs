use crate::system::arg::SystemArg;
use super::{Component, Entity, Row, World};

pub trait Command: Send + Sync + 'static {
    fn execute(self, world: &mut World);
}

pub struct BoxCommand(Box<dyn FnOnce(&mut World) + Send + Sync + 'static>);
impl BoxCommand {
    pub fn new<C: Command>(command: C) -> Self {
        BoxCommand(Box::new(move |world| command.execute(world)))
    }

    pub fn execute(self, world: &mut World) {
        (self.0)(world)
    }
}

#[derive(Default)]
pub struct Commands(Vec<BoxCommand>);

impl Commands {
    pub fn new() -> Self {
        Commands(Vec::new())
    }

    pub fn add<C: Command>(&mut self, command: C) {
        self.0.push(BoxCommand::new(command));
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

    pub fn drain(&mut self) -> Vec<BoxCommand> {
        std::mem::take(&mut self.0)
    }
}

pub struct CommandBuffer<'world, 'state> {
    commands: &'state mut Commands,
    _marker: std::marker::PhantomData<&'world ()>,
}

impl<'world, 'state> CommandBuffer<'world, 'state> {
    pub fn new(commands: &'state mut Commands) -> Self {
        CommandBuffer {
            commands,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn add<C: Command>(&mut self, command: C) {
        self.commands.add(command);
    }
}

unsafe impl SystemArg for CommandBuffer<'_, '_> {
    type Item<'world, 'state> = CommandBuffer<'world, 'state>;

    type State = Commands;

    fn init(_: &mut World) -> Self::State {
        Commands::new()
    }

    fn apply(state: &mut Self::State, world: &mut World) {
        for command in state.drain() {
            command.execute(world);
        }
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        _: super::WorldCell<'world>,
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        CommandBuffer::new(state)
    }
}

pub struct Spawner<'world, 'state> {
    world: &'world mut World,
    entities: &'state mut Vec<(Entity, Row)>,
    _marker: std::marker::PhantomData<&'state ()>,
}

impl<'world, 'state> Spawner<'world, 'state> {
    pub fn new(world: &'world mut World, entities: &'state mut Vec<(Entity, Row)>) -> Self {
        Spawner {
            world,
            entities,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn spawn(&mut self) -> Spawned<'world, 'state, '_> {
        let id = self.world.spawn();
        Spawned {
            id,
            components: Row::new(),
            spawner: self,
        }
    }
}

unsafe impl SystemArg for Spawner<'_, '_> {
    type Item<'world, 'state> = Spawner<'world, 'state>;

    type State = Vec<(Entity, Row)>;

    fn init(_: &mut World) -> Self::State {
        vec![]
    }

    unsafe fn get<'world, 'state>(
        state: &'state mut Self::State,
        mut world: super::WorldCell<'world>,
        _: &crate::system::SystemMeta,
    ) -> Self::Item<'world, 'state> {
        unsafe { Spawner::new(world.get_mut(), state) }
    }

    fn exclusive() -> bool {
        true
    }

    fn apply(state: &mut Self::State, world: &mut World) {
        for (entity, components) in state.drain(..) {
            world.add_components(entity, components);
        }
    }
}

pub struct Spawned<'world, 'state, 'spawner> {
    id: Entity,
    components: Row,
    spawner: &'spawner mut Spawner<'world, 'state>,
}

impl<'world, 'state, 'spawner> Spawned<'world, 'state, 'spawner> {
    pub fn with<C: Component>(mut self, component: C) -> Self {
        let id = unsafe { self.spawner.world.components().get_id_unchecked::<C>() };
        self.components.insert(id, component);
        self
    }

    pub fn finish(self) -> Entity {
        let id = self.id;
        self.spawner.entities.push((id, self.components));
        id
    }
}
