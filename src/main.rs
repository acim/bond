// Copyright 2020 Boban Acimovic
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

#[macro_use]
extern crate log;
use clap::Clap;
use futures::{stream, StreamExt, TryStreamExt};
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

    let mut nss = Vec::with_capacity(cfg.replica.len());
    let lp = ListParams::default().allow_bookmarks();
    for sr in cfg.replica.iter() {
        let (ns, _) = split_full_name(&sr.source);
        let a: Api<Secret> = Api::namespaced(client.clone(), ns);
        nss.push(watcher(a, lp.clone()).boxed());
    }

    let mut w = stream::select_all(nss);

    // let mut w = watcher(cms, lp).boxed();
    while let Some(event) = w.try_next().await? {
        match event {
            Applied(x) => info!("Applied: {}", full_name(&x)),
            Deleted(x) => info!("Deleted: {}", full_name(&x)),
            Restarted(x) => {
                for y in x.iter() {
                    info!("Restarted: {}", full_name(&y));

                    for i in cfg.replica.iter() {
                        if full_name(&y) == i.source {
                            info!("Trying to find destination {}", &i.destination[0]);
                            let (ns, n) = split_full_name(&i.destination[0]);
                            let cms: Api<Secret> = Api::namespaced(client.clone(), ns);
                            match cms.get(n).await {
                                Ok(s) => {
                                    info!("Found {}", full_name(&s));
                                    if y.data == s.data {
                                        info!("Exact data, nothing to do");
                                        continue;
                                    }
                                    if let Ok(ns) =
                                        new_secret(y, Meta::namespace(y).unwrap(), Meta::name(y))
                                    {
                                        let pp = PostParams::default();
                                        match cms.create(&pp, &ns).await {
                                            Ok(o) => {
                                                info!("Applied new secret: {}", full_name(&o));
                                                // wait for it..
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(5_000),
                                                );
                                            }
                                            Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
                                            Err(e) => error!("Error {}", e),
                                        }
                                    }
                                }
                                Err(e) => error!("Error {}", e),
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn new_secret(_si: &Secret, ns: String, n: String) -> Result<Secret, Error> {
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

fn full_name(s: &Secret) -> String {
    format!("{}/{}", Meta::namespace(s).unwrap(), Meta::name(s))
}

fn split_full_name<'a>(s: &'a String) -> (&'a str, &'a str) {
    let p: Vec<&str> = s.split("/").collect();
    (p[0], p[1])
}
