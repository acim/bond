#[macro_use]
extern crate log;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Secret;
use kube::{
    api::{Api, ListParams},
    Client,
};
use kube_runtime::watcher;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=debug");
    env_logger::init();

    let client = Client::try_default().await?;
    let namespace = std::env::var("NAMESPACE").unwrap_or("default".into());

    let cms: Api<Secret> = Api::namespaced(client, &namespace);
    let lp = ListParams::default().allow_bookmarks();

    let mut w = watcher(cms, lp).boxed();
    while let Some(event) = w.try_next().await? {
        match event {
            watcher::Event::Applied(x) => info!("Applied: {:?}", x.metadata.name.as_ref().unwrap()),
            watcher::Event::Deleted(x) => info!("Deleted: {:?}", x.metadata.name.as_ref().unwrap()),
            watcher::Event::Restarted(x) => {
                for y in x.iter() {
                    info!("Restarted: {:?}", y.metadata.name.as_ref().unwrap())
                }
            }
        }
    }
    Ok(())
}
