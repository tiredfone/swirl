pub mod draw;
pub mod era;
pub mod flow;
pub mod install;
pub mod launch;
pub mod net;
pub mod paths;
pub mod register;
pub mod state;
mod ui;

use era::Era;

pub fn run(era: Era) {
    ui::main(era);
}
