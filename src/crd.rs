use kube::{
    api::{Api, ListParams, Meta},
    Client, CustomResource,
};
use kube_runtime::{reflector, utils::try_flatten_applied, watcher};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "cert-manager.io",
    version = "v1",
    kind = "Certificate",
    namespaced
)]
#[kube(status = "CertificateStatus")]
pub struct CertificateSpec {
    #[serde(rename = "dnsNames")]
    dns_names: Vec<String>,

    #[serde(rename = "issuerRef")]
    issuer_ref: IssuerRef,

    #[serde(rename = "secretName")]
    secret_name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct IssuerRef {
    kind: String,
    name: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct CertificateStatus {
    conditions: Vec<Condition>,

    #[serde(rename = "notAfter")]
    not_after: String,

    #[serde(rename = "notBefore")]
    not_before: String,

    #[serde(rename = "renewalTime")]
    renewal_time: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct Condition {
    #[serde(rename = "lastTransitionTime")]
    last_transition_time: String,
    message: String,
    reason: String,
    status: String,
    #[serde(rename = "type")]
    condition_type: String,
}
