use kmiri_helper::*;

pub type Reachability = std::sync::Arc<[FunctionInstanceInfo]>;

pub fn collect() -> Reachability {
    let dir_target = std::env::var(ENV_INNER_DIR_TARGET)
        .unwrap_or_else(|err| panic!("The env var `{ENV_INNER_DIR_TARGET}` should be set: {err}"));
    let dir_target = std::path::PathBuf::from(dir_target);
    let analysis_json = dir_target.join(ANALYSIS_JSON);
    let file = std::fs::File::open(analysis_json).unwrap();
    todo!()
}
