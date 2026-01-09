use dorch_types::*;
use kube::CustomResourceExt;
use std::fs;

fn main() {
    let _ = fs::create_dir("../crds");
    fs::write(
        "../crds/dorch.beebs.dev_game_crd.yaml",
        serde_yaml::to_string(&Game::crd()).unwrap(),
    )
    .unwrap();
}
