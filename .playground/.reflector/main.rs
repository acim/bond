#[macro_use]
extern crate log;

use failure::{Error, Fail};
use futures::StreamExt;
use k8s_openapi::{
    api::core::v1::Secret,
    apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference},
};
use kube::{
    // api::{ListParams, Meta, Patch, PatchParams},
    api::{ListParams, Meta},
    Api,
    Client,
};
use kube_runtime::controller::{Context, Controller, ReconcilerAction};
// use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use tokio::time::Duration;

#[derive(Debug)]
struct MyError();

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SuperError is here!")
    }
}

impl std::error::Error for MyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self)
    }
}

fn object_to_owner_reference<K: Meta>(meta: ObjectMeta) -> Result<OwnerReference, Error> {
    Ok(OwnerReference {
        api_version: K::API_VERSION.to_string(),
        kind: K::KIND.to_string(),
        // name: meta.name.context(MissingObjectKey {
        //     name: ".metadata.name",
        // })?,
        // uid: meta.uid.context(MissingObjectKey {
        //     name: ".metadata.backtrace",
        // })?,
        ..OwnerReference::default()
    })
}

/// Controller triggers this whenever our main object or our children changed
async fn reconcile(generator: Secret, ctx: Context<Data>) -> Result<ReconcilerAction, MyError> {
    let client = ctx.get_ref().client.clone();

    let mut contents = BTreeMap::new();
    contents.insert("content".to_string(), generator.data);
    let cm = Secret {
        metadata: ObjectMeta {
            name: generator.metadata.name.clone(),
            owner_references: Some(vec![OwnerReference {
                controller: Some(true),
                ..object_to_owner_reference::<Secret>(generator.metadata.clone())?
            }]),
            ..ObjectMeta::default()
        },
        // data: Some(contents),
        ..Default::default()
    };
    let cm_api = Api::<Secret>::namespaced(
        client.clone(),
        generator
            .metadata
            .namespace
            .as_ref()
            .context(MissingObjectKey {
                name: ".metadata.namespace",
            })?,
    );
    cm_api
        .patch(
            cm.metadata.name.as_ref()..context(MissingObjectKey {
                name: ".metadata.name",
            })?,
            &PatchParams::apply("configmapgenerator.kube-rt.nullable.se"),
            &Patch::Apply(&cm),
        )
        .await?;
    Ok(ReconcilerAction {
        requeue_after: Some(Duration::from_secs(300)),
    })
}

/// The controller triggers this on reconcile errors
fn error_policy(_error: &MyError, _ctx: Context<Data>) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(1)),
    }
}

// Data we want access to in error/reconcile calls
struct Data {
    client: Client,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    std::env::set_var("RUST_LOG", "info,kube-runtime=debug,kube=debug");
    env_logger::init();
    let client = Client::try_default().await?;

    let cmgs = Api::<Secret>::all(client.clone());
    let cms = Api::<Secret>::all(client.clone());

    Controller::new(cmgs, ListParams::default())
        .owns(cms, ListParams::default())
        .run(reconcile, error_policy, Context::new(Data { client }))
        .for_each(|res| async move {
            match res {
                Ok(o) => info!("reconciled {:?}", o),
                Err(_) => warn!("reconcile failed"),
            }
        })
        .await;

    Ok(())
}
