pub struct TickResource {
    pub tick: u32,
}

impl TickResource {
    pub fn new() -> TickResource {
        TickResource { tick: 0 }
    }

    pub fn increment(&mut self) {
        self.tick += 1;
    }

    pub fn tick(&self) -> u32 {
        self.tick
    }
}
