use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct DefaultOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub setting: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepts: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInputFloat {
    #[serde(flatten)]
    pub default_options: DefaultOptions,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInputInteger {
    #[serde(flatten)]
    pub default_options: DefaultOptions,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInputBoolean {
    #[serde(flatten)]
    pub default_options: DefaultOptions,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInputSelect {
    #[serde(flatten)]
    pub default_options: DefaultOptions,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInputSeed {
    #[serde(flatten)]
    pub default_options: DefaultOptions,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<i32>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInputVec3 {
    #[serde(flatten)]
    pub default_options: DefaultOptions,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<f64>>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInputShape {
    #[serde(flatten)]
    pub default_options: DefaultOptions,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<f64>>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInputGeometry {
    #[serde(flatten)]
    pub default_options: DefaultOptions,
}

#[derive(Serialize, Deserialize)]
pub struct NodeInputPath {
    #[serde(flatten)]
    pub default_options: DefaultOptions,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum NodeInput {
    float(NodeInputFloat),
    integer(NodeInputInteger),
    boolean(NodeInputBoolean),
    select(NodeInputSelect),
    seed(NodeInputSeed),
    vec3(NodeInputVec3),
    shape(NodeInputShape),
    geometry(NodeInputGeometry),
    path(NodeInputPath),
}

#[derive(Serialize, Deserialize)]
pub struct NodeDefinitionMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct NodeDefinition {
    pub id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<NodeDefinitionMeta>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<HashMap<String, NodeInput>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<String>>,
}
