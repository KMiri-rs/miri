use kmiri_helper::*;

pub type Reachability = std::sync::Arc<[FunctionInstanceInfo]>;

pub fn collect() -> Option<Reachability> {
    let dir_target = std::env::var(ENV__KMIRI_DIR_TARGET).ok()?;
    let dir_target = std::path::PathBuf::from(dir_target);
    let analysis_json = dir_target.join(ANALYSIS_JSON);
    let file = std::fs::File::open(analysis_json).ok()?;
    serde_json::from_reader(file).ok()
}
