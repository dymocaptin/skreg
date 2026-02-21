//! skreg registry API server entry point.

use std::env;

fn main() {
    env_logger::init();
    let _database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    // Router and server startup wired in Task 9.
}
