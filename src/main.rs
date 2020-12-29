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
use k8s_openapi::{api::core::v1::Secret, Resource};
use kube::{
    api::{Api, ListParams, Meta, PostParams},
    Client,
};
use kube_runtime::{
    watcher,
    watcher::Event::{Applied, Deleted, Restarted},
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::Error;
use std::sync::{Arc, RwLock};

/// Kubernetes secrets replication across namespaces
#[derive(Clap, Debug)]
#[clap(name = "bond")]
struct Opt {
    /// Sets a custom config file
    #[clap(short, long, default_value = "bond")]
    config: String,
}

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
struct SecretReplica {
    source: String,
    destination: Vec<String>,
}

#[derive(Default, Debug, serde_derive::Serialize, serde_derive::Deserialize)]
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
    let mut api = KubeApi::new(client);

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
                            for d in i.destination.iter() {
                                info!("Getting destination secret {}", &d);
                                match api.get(&d).await {
                                    Ok(s) => {
                                        info!("Found {}", full_name(&s));
                                        if y.data == s.data {
                                            info!("Secret up to date");
                                            continue;
                                        }
                                        let pp = PostParams::default();
                                        let new = new_secret(d, &s).unwrap();
                                        let (namespace, _) = split_full_name(d);
                                        match api.create(namespace, &pp, &new).await {
                                            Ok(o) => {
                                                info!("Created new secret: {}", full_name(&o));
                                                // wait for it..
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(5_000),
                                                );
                                            }
                                            Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
                                            Err(e) => error!("Error {}", e),
                                        }
                                    }
                                    Err(_) => {
                                        let pp = PostParams::default();
                                        let new = new_secret(d, &y).unwrap();
                                        let (namespace, _) = split_full_name(d);
                                        match api.create(namespace, &pp, &new).await {
                                            Ok(o) => {
                                                info!("Created new secret: {}", full_name(&o));
                                                // wait for it..
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(5_000),
                                                );
                                            }
                                            Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
                                            Err(e) => error!("{}", e),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn new_secret(full_name: &str, secret: &Secret) -> Result<Secret, Error> {
    let (namespace, name) = split_full_name(full_name);
    let so: Secret = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "annotations": {
                "meta.ectobit.com/managed-by": "bond",
            }
        },
        "type": secret.type_,
        "data": secret.data
    }))?;
    Ok(so)
}

fn full_name(s: &Secret) -> String {
    format!("{}/{}", Meta::namespace(s).unwrap(), Meta::name(s))
}

fn split_full_name(s: &str) -> (&str, &str) {
    let parts: Vec<&str> = s.split('/').collect();
    (parts[0], parts[1])
}

struct KubeApi<'a, T> {
    client: Client,
    lock: Arc<RwLock<HashMap<&'a str, Api<T>>>>,
}

impl<'a, T> KubeApi<'a, T>
where
    T: Resource + Clone + DeserializeOwned + Meta + Serialize + std::fmt::Debug,
{
    fn new(client: Client) -> KubeApi<'a, T> {
        KubeApi {
            client,
            lock: Arc::new(RwLock::new(HashMap::with_capacity(1))),
        }
    }

    async fn get(&mut self, full_name: &'a str) -> Result<T, kube::Error> {
        let (namespace, name) = split_full_name(full_name);
        let lock = Arc::clone(&self.lock);
        let map = lock.read().unwrap();
        match map.get(namespace) {
            Some(api) => return api.get(name).await,
            None => {
                drop(map);
                let lock = Arc::clone(&self.lock);
                let mut map = lock.write().unwrap();
                let new_api = Api::<T>::namespaced(self.client.clone(), namespace);
                let api = map.entry(namespace).or_insert(new_api);
                return api.get(name).await;
            }
        }
    }

    async fn create(
        &self,
        namespace: &'a str,
        pp: &PostParams,
        data: &T,
    ) -> Result<T, kube::Error> {
        info!("{} {:#?}", namespace, data);
        let lock = Arc::clone(&self.lock);
        let map = lock.write().unwrap();
        match map.get(namespace) {
            Some(api) => return api.create(pp, data).await,
            None => {
                drop(map);
                let lock = Arc::clone(&self.lock);
                let mut map = lock.write().unwrap();
                let new_api = Api::<T>::namespaced(self.client.clone(), namespace);
                let api = map.entry(namespace).or_insert(new_api);
                return api.create(pp, data).await;
            }
        }
    }
}
