use system::query::{Added, Query, QueryState};
use world::Component;

pub mod core;
pub mod system;
pub mod world;

fn main() {
    let mut world = world::World::new();
    world.register::<Name>();
    world.register::<Age>();

    let entity = world.spawn();
    world.add_component(entity, Age(30));
    world.add_component(entity, Name("Alice".to_string()));

    let state = QueryState::new(&world);
    let query = Query::<(&Age, &Name), Added<Age>>::new(&world, &state);
    for (age, name) in query.iter() {
        println!("Query found age: {:?}", age);
        println!("Query found name: {:?}", name);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Age(u32);
impl Component for Age {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);
impl Component for Name {}
