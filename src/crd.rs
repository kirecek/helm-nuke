use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "k8s.kirecek.dev",
    version = "v1",
    kind = "HelmNuke",
    plural = "helmnukes",
    status = "HelmNukeStatus",
    namespaced
)]
pub struct HelmNukeSpec {
    pub ttl: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct HelmNukeStatus {
    pub expiration_timestamp: Option<String>,
}
