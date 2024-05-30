use ripple_sdk::api::manifest::device_manifest::DeviceManifest;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Please provide a JSON file as an argument.");
        return;
    }

    let file_contents = fs::read_to_string(&args[1]).expect("Could not read file");
    let manifest: DeviceManifest =
        serde_json::from_str(&file_contents).expect("Could not deserialize JSON");

    let default_manifest = DeviceManifest::default();

    println!("Manifest: {:?}", manifest);

    // if manifest.field1 == default_manifest.field1 {
    //     println!("field1 has the default value");
    // }
    // if manifest.field2 == default_manifest.field2 {
    //     println!("field2 has the default value");
    // }
    // if manifest.field3 == default_manifest.field3 {
    //     println!("field3 has the default value");
    // }
}
