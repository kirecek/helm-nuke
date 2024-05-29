use anyhow::Result;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use kube::{
    api::{Api, Patch, PatchParams, ResourceExt},
    runtime::{
        controller::{Action, Controller},
        watcher,
    },
    Client, Error,
};
use serde_json::json;
use std::process::{Command, Output};
use std::sync::Arc;
use tokio::time::Duration;
use tracing::*;

mod crd;
use crd::{HelmNuke, HelmNukeStatus};

struct Data {
    client: Client,
}

#[tokio::main]
async fn main() -> Result<()> {
    info!("Starting...");

    let client = Client::try_default().await?;
    let crd_api = Api::<HelmNuke>::all(client.clone());

    Controller::new(crd_api, watcher::Config::default())
        .shutdown_on_signal()
        .run(reconcile, error_policy, Arc::new(Data { client }))
        .for_each(|res| async move {
            match res {
                Ok(o) => info!("reconciled {:?}", o),
                Err(e) => warn!("reconcile failed: {}", e),
            }
        })
        .await;

    info!("Bye");
    Ok(())
}

async fn reconcile(object: Arc<HelmNuke>, ctx: Arc<Data>) -> Result<Action, Error> {
    let crd_api = Api::<HelmNuke>::default_namespaced(ctx.client.clone());

    let ttl: Duration = humantime::parse_duration(&object.spec.ttl).unwrap().into();

    let mut is_expired = false;

    match object.status.as_ref() {
        Some(status) => match &status.expiration_timestamp {
            Some(ts) => {
                let exp_time = DateTime::parse_from_rfc3339(&ts)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap();

                if exp_time <= Utc::now() {
                    is_expired = true;
                }
            }
            _ => {}
        },
        None => {
            let expiration_timestamp =
                chrono::Utc::now() + chrono::Duration::from_std(ttl).unwrap();

            let status = json!({
                "status": HelmNukeStatus {
                    expiration_timestamp: Some(expiration_timestamp.to_rfc3339()),
                }
            });

            crd_api
                .patch_status(
                    &object.name_any(),
                    &PatchParams::default(),
                    &Patch::Merge(&status),
                )
                .await?;

            return Ok(Action::requeue(ttl + Duration::from_secs(10)));
        }
    }

    let annotations = object.metadata.annotations.as_ref().unwrap();

    match (
        annotations.get("meta.helm.sh/release-name"),
        annotations.get("meta.helm.sh/release-namespace"),
    ) {
        (Some(name), Some(namespace)) => {
            if is_expired {
                info!("uninstalling helm release {}/{}", namespace, name);
                helm_uninstall(name, namespace).unwrap();
            }
        }
        _ => {
            warn!("misssing helm release metadata for {:?}", object.name_any());
        }
    }

    Ok(Action::requeue(Duration::from_secs(180)))
}

fn helm_uninstall(
    release_name: &str,
    namespace: &str,
) -> Result<Output, Box<dyn std::error::Error>> {
    let mut command = Command::new("helm");
    command.arg("uninstall").arg(release_name);
    command.arg("--namespace").arg(namespace);

    let output = command.output()?;

    if !output.status.success() {
        return Err(format!(
            "Failed to uninstall helm release: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    Ok(output)
}

fn error_policy(_object: Arc<HelmNuke>, _error: &Error, _ctx: Arc<Data>) -> Action {
    Action::requeue(Duration::from_secs(30))
}
