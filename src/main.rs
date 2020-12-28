// Copyright 2020 Boban Acimovic
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

#[macro_use]
extern crate log;
use clap::Clap;
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Secret;
use kube::{
    api::{Api, ListParams},
    Client,
};
use kube_runtime::{
    watcher,
    watcher::Event::{Applied, Deleted, Restarted},
};
use serde_derive::{Deserialize, Serialize};

/// Kubernetes secrets replication across namespaces
#[derive(Clap, Debug)]
#[clap(name = "bond")]
struct Opt {
    /// Sets a custom config file
    #[clap(short, long, default_value = "bond")]
    config: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SecretReplica {
    source: String,
    destination: Vec<String>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct Config {
    replica: Vec<SecretReplica>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=debug");
    env_logger::init();

    let opt: &Opt = &Opt::parse();
    info!("Options: {:#?}", opt);

    let cfg: Config = confy::load(opt.config.as_str()).unwrap();
    info!("Config: {:#?}", cfg);

    let client = Client::try_default().await?;

    // let namespace = std::env::var("NAMESPACE").unwrap_or_else(|_| "default".into());
    // let cms: Api<Secret> = Api::namespaced(client, &namespace);

    let cms: Api<Secret> = Api::all(client);
    let lp = ListParams::default().allow_bookmarks();

    let mut w = watcher(cms, lp).boxed();
    while let Some(event) = w.try_next().await? {
        match event {
            Applied(x) => info!("Applied: {:?}", x.metadata.name.as_ref().unwrap()),
            Deleted(x) => info!("Deleted: {:?}", x.metadata.name.as_ref().unwrap()),
            Restarted(x) => {
                for y in x.iter() {
                    info!(
                        "Restarted: {}/{}",
                        y.metadata.namespace.as_ref().unwrap(),
                        y.metadata.name.as_ref().unwrap()
                    )
                }
            }
        }
    }
    Ok(())
}
