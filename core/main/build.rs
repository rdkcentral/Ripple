extern crate vergen;
use vergen::{vergen, ConstantsFlags};

fn main() {
    /*generate consts in version.rs and blow up if something goes wrong */
    assert!(vergen(ConstantsFlags::all()).is_ok());
}
