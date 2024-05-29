use kube::CustomResourceExt;

mod crd;
use crd::HelmNuke;

fn main() {
    print!("{}", serde_yaml::to_string(&HelmNuke::crd()).unwrap())
}
