use k8s_openapi::{api::core::v1::Secret, Resource};
use kube::client::Status;
use kube::{
    api::{Api, DeleteParams, Meta, PostParams},
    Client,
};
use serde::{de::DeserializeOwned, Serialize};

pub struct KubeApi {
    client: Client,
}

#[allow(dead_code)]
impl KubeApi {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Retrieve namespaced Kubernetes resource consuming full name.
    pub async fn get<T, U>(&mut self, full_name: U) -> Result<T, kube::Error>
    where
        T: Resource + Clone + DeserializeOwned + Meta + Serialize + std::fmt::Debug,
        U: AsRef<str>,
    {
        let (namespace, name) = split_full_name(full_name.as_ref());
        let api = Api::<T>::namespaced(self.client.clone(), namespace);
        api.get(name).await
    }

    /// Create namespaced Kubernetes resource consuming full name and resource.
    pub async fn create<T, U>(&self, full_name: U, data: &T) -> Result<T, kube::Error>
    where
        T: Resource + Clone + DeserializeOwned + Meta + Serialize + std::fmt::Debug,
        U: AsRef<str>,
    {
        let (namespace, _) = split_full_name(full_name.as_ref());
        let api = Api::<T>::namespaced(self.client.clone(), namespace);
        api.create(&PostParams::default(), data).await
    }

    // Delete namespaced Kubernetes resource consuming full name.
    // pub async fn delete<T, U>(&self, full_name: U) -> Result<either::Either<T, Status>, kube::Error>
    // where
    //     T: Resource + Clone + DeserializeOwned + Meta + Serialize + std::fmt::Debug,
    //     U: AsRef<str>,
    // {
    //     let (namespace, name) = split_full_name(full_name.as_ref());
    //     let api = Api::<T>::namespaced(self.client.clone(), namespace);
    //     api.delete(name, &DeleteParams::default()).await
    // }
}

pub fn full_name(s: &Secret) -> String {
    format!("{}/{}", Meta::namespace(s).unwrap(), Meta::name(s))
}

pub fn split_full_name(s: &str) -> (&str, &str) {
    let parts: Vec<&str> = s.split('/').collect();
    (parts[0], parts[1])
}
