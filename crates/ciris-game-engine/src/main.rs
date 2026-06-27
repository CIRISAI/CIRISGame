//! CIRISGame binary entry point. Delegates to [`ciris_game_engine::run`], which
//! opens a window under the render features or runs headless otherwise.

fn main() {
    ciris_game_engine::run();
}
