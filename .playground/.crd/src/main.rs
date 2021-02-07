#[macro_use]
extern crate log;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{
    api::{Api, ListParams, Meta},
    Client,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=debug");
    env_logger::init();
    let client = Client::try_default().await?;
    let crds: Api<CustomResourceDefinition> = Api::all(client);
    let lp = ListParams::default()
        .fields("metadata.name=certificates.cert-manager.io")
        .timeout(20);
    match crds.list(&lp).await {
        Ok(crd) => {
            for i in crd {
                info!("found {}", Meta::name(&i))
            }
        }
        Err(e) => error!("error {:?}", e),
    }

    Ok(())
}
