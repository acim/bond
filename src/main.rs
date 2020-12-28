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
    api::{Api, ListParams, Meta, PostParams},
    Client,
};
use kube_runtime::{
    watcher,
    watcher::Event::{Applied, Deleted, Restarted},
};
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use std::io::Error;

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

    let cms = Api::<Secret>::all(client);
    let cms2 = cms.clone();
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
                    );

                    if false {
                        let ns = new_secret(
                            y,
                            y.metadata.namespace.as_ref().unwrap(),
                            y.metadata.name.as_ref().unwrap(),
                        )
                        .unwrap();

                        let pp = PostParams::default();
                        match cms2.create(&pp, &ns).await {
                            Ok(o) => {
                                let tns = Meta::namespace(&o);
                                let tn = Meta::name(&o);
                                assert_eq!(Meta::name(&ns), tn);
                                assert_eq!(Meta::namespace(&ns), tns);
                                info!("Applied new secret: {}/{}", tns.unwrap(), tn);
                                // wait for it..
                                std::thread::sleep(std::time::Duration::from_millis(5_000));
                            }
                            Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
                            Err(_e) => {} //return Err(e.into()), // any other case is probably bad
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn new_secret(_si: &Secret, ns: &str, n: &str) -> Result<Secret, Error> {
    let so: Secret = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": n,
            "namespace": ns,
            "annotations": {
                "k8s.ectobit.com/bond": "true",
            }
        },
        "data": {}
    }))?;
    Ok(so)
}
