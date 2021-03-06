#[macro_use]
extern crate log;
use futures::{StreamExt, TryStreamExt};
use kube::{
    api::{Api, ListParams, Meta},
    Client, CustomResource,
};
use kube_runtime::{reflector, utils::try_flatten_applied, watcher};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=debug");
    env_logger::init();
    let client = Client::try_default().await?;
    let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

    let store = reflector::store::Writer::<Certificate>::default();
    let reader = store.as_reader();
    let certs: Api<Certificate> = Api::namespaced(client, &namespace);
    let lp = ListParams::default().timeout(20);
    let rf = reflector::<Certificate, _>(store, watcher(certs, lp));

    tokio::spawn(async move {
        loop {
            // Periodically read our state
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            // let crds = reader.state().iter().map(Meta::name).collect::<Vec<_>>();
            // info!("Current crds: {:?}", crds);
            for crd in reader.state() {
                info!(
                    "CRD: {} secret: {} status: {}",
                    Meta::name(&crd),
                    crd.spec.secret_name,
                    crd.status.unwrap().conditions[0].status
                )
            }
        }
    });
    let mut rfa = try_flatten_applied(rf).boxed();
    while let Some(event) = rfa.try_next().await? {
        info!(
            "Applied: {} secret: {} status: {}",
            Meta::name(&event),
            event.spec.secret_name,
            event.status.unwrap().conditions[0].status,
        );
    }
    Ok(())
}
