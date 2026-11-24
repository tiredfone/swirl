#[cfg(not(target_arch = "wasm32"))]
mod imp {
    pub struct Clock(std::time::Instant);

    impl Clock {
        pub fn new() -> Clock {
            Clock(std::time::Instant::now())
        }

        pub fn elapsed(&self) -> f64 {
            self.0.elapsed().as_secs_f64()
        }
    }

    pub fn wall() -> f64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
    }

    pub fn set_now(_: f64) {}

    pub fn tick() -> f64 {
        wall()
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NOW: AtomicU64 = AtomicU64::new(0);
    static WALL: AtomicU64 = AtomicU64::new(0);

    pub struct Clock(f64);

    impl Clock {
        pub fn new() -> Clock {
            Clock(f64::from_bits(NOW.load(Ordering::Relaxed)))
        }

        pub fn elapsed(&self) -> f64 {
            f64::from_bits(NOW.load(Ordering::Relaxed)) - self.0
        }
    }

    pub fn wall() -> f64 {
        f64::from_bits(WALL.load(Ordering::Relaxed))
    }

    pub fn set_now(secs: f64) {
        NOW.store(secs.to_bits(), Ordering::Relaxed);
    }

    pub fn tick() -> f64 {
        wall() + f64::from_bits(NOW.load(Ordering::Relaxed))
    }

    pub fn set_wall(secs: f64) {
        WALL.store(secs.to_bits(), Ordering::Relaxed);
    }
}

pub use imp::*;
