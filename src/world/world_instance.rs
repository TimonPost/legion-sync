use legion::{Schedule, World, Resources};

pub struct WorldInstance {
    pub(crate) world: World,
    pub(crate) schedule: Schedule,
}

impl WorldInstance {
    pub fn new(world: World, schedule: Schedule) -> WorldInstance {
        WorldInstance { world, schedule }
    }

    pub fn execute(&mut self, resources: &mut Resources) {
        self.schedule.execute(&mut self.world, resources);
    }
}
