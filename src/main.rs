// Copyright 2020 Boban Acimovic
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//!
#![warn(missing_debug_implementations, rust_2018_idioms, missing_docs)]

#[macro_use]
extern crate log;

use clap::Clap;
use futures::{stream, StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Secret;
use kube::{
    api::{Api, ListParams, Meta},
    Client,
};
use kube_runtime::{
    utils::try_flatten_applied,
    watcher,
    // watcher::Event::{Applied, Deleted, Restarted},
};
// use serde_json::json;
// use std::io::Error;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use warp::Filter;

mod client;
mod crd;

// const LABEL: &str = "app.kubernetes.io/managed-by";

/// Kubernetes secrets replication across namespaces
#[derive(Clap, Debug)]
#[clap(name = "bond")]
struct Opt {
    #[clap(short, long, default_value = "3000")]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=debug");
    json_env_logger::init();

    let opt = &Opt::parse();
    info!("options: {:?}", opt);

    // Run health service
    let port = opt.port;
    tokio::spawn(async move { serve(port).await });

    let client = Client::try_default().await?;

    let api = client::KubeApi::new(client.clone());

    let secrets: Api<Secret> = Api::namespaced(client.clone(), "default");
    let secrets_watcher = watcher(secrets, ListParams::default().labels("owner!=helm"));

    let mut combo_stream = stream::select_all(vec![try_flatten_applied(secrets_watcher)
        .map_ok(Watched::Secret)
        .boxed()]);

    if api
        .is_crd_installed::<CustomResourceDefinition>("metadata.name=certificates.cert-manager.io")
        .await
    {
        let certs: Api<crd::Certificate> = Api::namespaced(client, "default");
        let certs_watcher = watcher(certs, ListParams::default());
        combo_stream.push(
            try_flatten_applied(certs_watcher)
                .map_ok(Watched::Certificate)
                .boxed(),
        )
    }

    while let Some(event) = combo_stream.try_next().await? {
        match event {
            Watched::Secret(s) => info!("Got secret: {}", Meta::name(&s)),
            Watched::Certificate(c) => info!("Got certificate: {}", Meta::name(&c)),
            // Applied(x) => info!("Applied: {}", client::full_name(&x)),
            // Deleted(x) => info!("Deleted: {}", client::full_name(&x)),
            // Restarted(x) => {
            //     for y in x.iter() {
            //         info!("Restarted: {}", client::full_name(&y));

            //         for i in cfg.replica.iter() {
            //             if client::full_name(&y) == i.source {
            //                 for d in i.destination.iter() {
            //                     info!("Getting destination secret {}", &d);
            //                     match api.get(&d).await {
            //                         Ok(s) => {
            //                             info!("Found {}", client::full_name(&s));
            //                             if y.data == s.data {
            //                                 info!("Secret up to date");
            //                                 continue;
            //                             }
            //                             let new = new_secret(d, &s).unwrap();
            //                             match api.create(d, &new).await {
            //                                 Ok(o) => {
            //                                     info!(
            //                                         "Created new secret: {}",
            //                                         client::full_name(&o)
            //                                     );
            //                                     // wait for it..
            //                                     std::thread::sleep(
            //                                         std::time::Duration::from_millis(5_000),
            //                                     );
            //                                 }
            //                                 Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
            //                                 Err(e) => error!("Error {}", e),
            //                             }
            //                         }
            //                         Err(_) => {
            //                             let new = new_secret(d, &y).unwrap();
            //                             match api.create(d, &new).await {
            //                                 Ok(o) => {
            //                                     info!(
            //                                         "Created new secret: {}",
            //                                         client::full_name(&o)
            //                                     );
            //                                     // wait for it..
            //                                     std::thread::sleep(
            //                                         std::time::Duration::from_millis(5_000),
            //                                     );
            //                                 }
            //                                 Err(kube::Error::Api(ae)) => assert_eq!(ae.code, 409), // if you skipped delete, for instance
            //                                 Err(e) => error!("{}", e),
            //                             }
            //                         }
            //                     }
            //                 }
            //             }
            //         }
            //     }
            // }
        }
    }
    Ok(())
}

// fn new_secret(full_name: &str, secret: &Secret) -> Result<Secret, Error> {
//     let (namespace, name) = client::split_full_name(full_name);
//     let so: Secret = serde_json::from_value(json!({
//         "apiVersion": "v1",
//         "kind": "Secret",
//         "metadata": {
//             "name": name,
//             "namespace": namespace,
//             "labels": {
//                 LABEL: "bond",
//             }
//         },
//         "type": secret.type_,
//         "data": secret.data
//     }))?;
//     Ok(so)
// }

async fn serve(port: u16) {
    let live = warp::path!("live").map(|| r#"{"status":"OK"}"#);

    warp::serve(live).run(([0, 0, 0, 0], port)).await;
}

// #[test]
// fn it_works() {
//     let mut s: Secret = Default::default();
//     let mut a = std::collections::BTreeMap::<String, String>::new();
//     a.insert(LABEL.to_string(), "bond".to_string());
//     s.metadata.name = Some("bar".to_string());
//     s.metadata.namespace = Some("foo".to_string());
//     s.metadata.labels = Some(a);
//     assert_eq!(s, new_secret("foo/bar", &s).unwrap())
// }

enum Watched {
    Secret(Secret),
    Certificate(crd::Certificate),
}
