
pub fn convert_path(path: &str)->String{
    if cfg!(target_os = "windows") {
        path.to_owned().replace("/", "\\")
    }
    else {
        path.to_owned()
    }
}