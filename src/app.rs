use crate::{
    system::{
        executor::RunMode,
        schedule::{Phase, Schedule, Systems},
    },
    world::World,
};

pub struct AppBuilder {
    world: World,
    schedule: Schedule,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            schedule: Schedule::new(RunMode::Sequential),
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    pub fn schedule_mut(&mut self) -> &mut Schedule {
        &mut self.schedule
    }

    pub fn build(mut self) -> App {
        let systems = self.schedule.build(&mut self.world);

        App {
            world: self.world,
            systems,
        }
    }
}

pub struct App {
    world: World,
    systems: Systems,
}

impl App {
    pub fn new() -> AppBuilder {
        AppBuilder::new()
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn systems(&self) -> &Systems {
        &self.systems
    }

    pub fn run(&mut self, phase: impl Phase) {
        self.systems.run(&mut self.world, phase);
        self.world.update();
    }
}
