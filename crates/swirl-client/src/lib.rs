use swirl_net::world::World;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Scene {
    world: World,
}

#[wasm_bindgen]
impl Scene {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Scene {
        Scene { world: World::new() }
    }

    pub fn apply(&mut self, text: &str) -> String {
        self.world.apply(text).unwrap_or_default()
    }

    pub fn ready(&self) -> bool {
        self.world.ready
    }

    pub fn era(&self) -> i32 {
        self.world.era
    }

    pub fn part_count(&self) -> usize {
        self.world.parts().len()
    }

    pub fn mesh_part_count(&self) -> usize {
        self.world.parts().iter().filter(|p| p.mesh.is_some()).count()
    }

    pub fn instance_count(&self) -> usize {
        self.world.insts.len()
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
mod gl;

#[cfg(target_arch = "wasm32")]
mod clientchars;

#[cfg(target_arch = "wasm32")]
mod gui;

#[cfg(target_arch = "wasm32")]
mod luavm;

#[cfg(target_arch = "wasm32")]
pub use gl::Client;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[wasm_bindgen]
pub fn client_version() -> String {
    version().to_string()
}
