use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::config::ComfyUiSection;

#[derive(Debug, Clone, Default)]
pub struct ComfyPromptOverrides<'a> {
    pub prompt: Option<&'a str>,
    pub negative_prompt: Option<&'a str>,
    pub input_image: Option<&'a str>,
    pub denoise: Option<f64>,
}

pub fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn resolve_project_path(path: &str) -> PathBuf {
    let expanded = expand_user_path(path);
    if expanded.is_absolute() {
        expanded
    } else {
        project_root().join(expanded)
    }
}

pub fn expand_user_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    if let Some(rest) = path.strip_prefix("%USERPROFILE%\\") {
        if let Ok(home) = std::env::var("USERPROFILE") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

pub fn comfyui_endpoint(base_url: &str) -> (String, u16) {
    reqwest::Url::parse(base_url)
        .ok()
        .map(|url| {
            let host = url.host_str().unwrap_or("127.0.0.1").to_string();
            let port = url.port_or_known_default().unwrap_or(8188);
            (host, port)
        })
        .unwrap_or_else(|| ("127.0.0.1".into(), 8188))
}

pub fn build_comfyui_launch_command(config: &ComfyUiSection) -> Option<(String, PathBuf)> {
    if config.root_dir.trim().is_empty() {
        return None;
    }
    let root_dir = resolve_project_path(&config.root_dir);

    let (host, port) = comfyui_endpoint(&config.base_url);
    let python = if config.python_path.trim().is_empty() {
        "python".to_string()
    } else {
        expand_user_path(&config.python_path)
            .to_string_lossy()
            .to_string()
    };

    let command = if config.launch_command.trim().is_empty() {
        format!("{python} main.py --listen {host} --port {port}")
    } else {
        config
            .launch_command
            .replace("{python}", &python)
            .replace("{root_dir}", &root_dir.to_string_lossy())
            .replace("{host}", &host)
            .replace("{port}", &port.to_string())
    };

    Some((command, root_dir))
}

pub fn screenshot_metadata(enabled: bool, retention: u32) -> Value {
    json!({
        "enabled": enabled,
        "retention": retention,
        "capture_available": false,
        "reason": "screenshot capture is disabled until an explicit platform capture backend is added"
    })
}

pub fn load_comfyui_prompt_with_overrides(
    workflow_path: &str,
    overrides: ComfyPromptOverrides<'_>,
) -> Result<Value, String> {
    let path = resolve_project_path(workflow_path);
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read workflow {}: {}", path.display(), e))?;
    let mut workflow: Value =
        serde_json::from_str(&raw).map_err(|e| format!("workflow JSON parse error: {}", e))?;

    if looks_like_api_workflow(&workflow) {
        apply_api_prompt_overrides(&mut workflow, overrides.prompt, overrides.negative_prompt);
        apply_api_image_overrides(&mut workflow, overrides.input_image, overrides.denoise)?;
        return Ok(workflow);
    }

    apply_ui_prompt_overrides(&mut workflow, overrides.prompt, overrides.negative_prompt);
    let mut prompt = ui_workflow_to_api_prompt(&workflow)?;
    apply_api_image_overrides(&mut prompt, overrides.input_image, overrides.denoise)?;
    Ok(prompt)
}

fn looks_like_api_workflow(value: &Value) -> bool {
    value.as_object().is_some_and(|obj| {
        obj.values()
            .any(|node| node.get("class_type").and_then(Value::as_str).is_some())
    })
}

fn apply_api_prompt_overrides(workflow: &mut Value, prompt: Option<&str>, negative: Option<&str>) {
    let Some(nodes) = workflow.as_object_mut() else {
        return;
    };
    for node in nodes.values_mut() {
        if node.get("class_type").and_then(Value::as_str) != Some("CLIPTextEncode") {
            continue;
        }
        let Some(inputs) = node.get_mut("inputs").and_then(Value::as_object_mut) else {
            continue;
        };
        let current = inputs.get("text").and_then(Value::as_str).unwrap_or("");
        if is_negative_prompt_text(current) {
            if let Some(negative) = negative {
                inputs.insert("text".into(), Value::String(negative.to_string()));
            }
        } else if let Some(prompt) = prompt {
            inputs.insert("text".into(), Value::String(prompt.to_string()));
        }
    }
}

fn apply_api_image_overrides(
    workflow: &mut Value,
    input_image: Option<&str>,
    denoise: Option<f64>,
) -> Result<(), String> {
    let Some(nodes) = workflow.as_object_mut() else {
        return Ok(());
    };
    let mut image_applied = input_image.is_none();
    for node in nodes.values_mut() {
        let class_type = node
            .get("class_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let Some(inputs) = node.get_mut("inputs").and_then(Value::as_object_mut) else {
            continue;
        };
        if let Some(image) = input_image {
            if class_type == "LoadImage" {
                inputs.insert("image".into(), Value::String(image.to_string()));
                image_applied = true;
            }
        }
        if let Some(denoise) = denoise {
            if class_type == "KSampler" {
                inputs.insert("denoise".into(), json!(denoise));
            }
        }
    }
    if !image_applied {
        return Err("image+text generation requires a workflow with a LoadImage node".into());
    }
    Ok(())
}

fn apply_ui_prompt_overrides(workflow: &mut Value, prompt: Option<&str>, negative: Option<&str>) {
    let Some(nodes) = workflow.get_mut("nodes").and_then(Value::as_array_mut) else {
        return;
    };
    for node in nodes {
        if node.get("type").and_then(Value::as_str) != Some("PrimitiveNode") {
            continue;
        }
        let title = node
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let replacement = if title.contains("negative") {
            negative
        } else if title.contains("positive") {
            prompt
        } else {
            None
        };
        if let Some(text) = replacement {
            if let Some(values) = node.get_mut("widgets_values").and_then(Value::as_array_mut) {
                if !values.is_empty() {
                    values[0] = Value::String(text.to_string());
                }
            }
        }
    }
}

fn is_negative_prompt_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("watermark") || lower.contains("negative") || lower.contains("bad quality")
}

fn ui_workflow_to_api_prompt(workflow: &Value) -> Result<Value, String> {
    let nodes = workflow
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "workflow does not contain nodes".to_string())?;
    let links = workflow
        .get("links")
        .and_then(Value::as_array)
        .ok_or_else(|| "workflow does not contain links".to_string())?;

    let node_by_id: HashMap<i64, &Value> = nodes
        .iter()
        .filter_map(|node| node.get("id").and_then(Value::as_i64).map(|id| (id, node)))
        .collect();
    let link_by_id: HashMap<i64, (i64, i64)> = links
        .iter()
        .filter_map(|link| {
            let parts = link.as_array()?;
            Some((
                parts.first()?.as_i64()?,
                (parts.get(1)?.as_i64()?, parts.get(2)?.as_i64()?),
            ))
        })
        .collect();

    let mut prompt = Map::new();
    for node in nodes {
        let Some(id) = node.get("id").and_then(Value::as_i64) else {
            continue;
        };
        let class_type = node
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("node {id} has no type"))?;
        if matches!(class_type, "Note" | "MarkdownNote" | "PrimitiveNode") {
            continue;
        }

        let mut inputs = widget_inputs_for_node(class_type, node);
        if let Some(input_defs) = node.get("inputs").and_then(Value::as_array) {
            for input_def in input_defs {
                let Some(name) = input_def.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let Some(link_id) = input_def.get("link").and_then(Value::as_i64) else {
                    continue;
                };
                let Some((source_id, source_slot)) = link_by_id.get(&link_id) else {
                    continue;
                };
                let Some(source) = node_by_id.get(source_id) else {
                    continue;
                };
                if source.get("type").and_then(Value::as_str) == Some("PrimitiveNode") {
                    if let Some(value) = source
                        .get("widgets_values")
                        .and_then(Value::as_array)
                        .and_then(|values| values.first())
                    {
                        inputs.insert(name.to_string(), value.clone());
                    }
                } else {
                    inputs.insert(
                        name.to_string(),
                        Value::Array(vec![
                            Value::String(source_id.to_string()),
                            Value::Number((*source_slot).into()),
                        ]),
                    );
                }
            }
        }

        prompt.insert(
            id.to_string(),
            json!({
                "class_type": class_type,
                "inputs": inputs,
            }),
        );
    }

    Ok(Value::Object(prompt))
}

fn widget_inputs_for_node(class_type: &str, node: &Value) -> Map<String, Value> {
    let mut inputs = Map::new();
    let values = node
        .get("widgets_values")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let names: &[&str] = match class_type {
        "EmptyLatentImage" => &["width", "height", "batch_size"],
        "CheckpointLoaderSimple" => &["ckpt_name"],
        "CLIPTextEncode" => &["text"],
        "LoadImage" => &["image", "upload"],
        "KSampler" => &[
            "seed",
            "control_after_generate",
            "steps",
            "cfg",
            "sampler_name",
            "scheduler",
            "denoise",
        ],
        "KSamplerAdvanced" => &[
            "add_noise",
            "noise_seed",
            "control_after_generate",
            "steps",
            "cfg",
            "sampler_name",
            "scheduler",
            "start_at_step",
            "end_at_step",
            "return_with_leftover_noise",
        ],
        "SaveImage" => &["filename_prefix"],
        _ => &[],
    };

    for (index, name) in names.iter().enumerate() {
        if let Some(value) = values.get(index) {
            inputs.insert((*name).to_string(), value.clone());
        }
    }
    inputs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_sdxl_ui_workflow_to_api_prompt() {
        let path = project_root().join("assets/workflows/sdxl.json");
        if !path.exists() {
            return;
        }
        let prompt = load_comfyui_prompt_with_overrides(
            path.to_string_lossy().as_ref(),
            ComfyPromptOverrides {
                prompt: Some("a small brass astrolabe on a desk"),
                negative_prompt: Some("text, watermark"),
                input_image: None,
                denoise: None,
            },
        )
        .unwrap();
        let obj = prompt.as_object().unwrap();
        assert!(obj
            .values()
            .any(|node| { node.get("class_type").and_then(Value::as_str) == Some("SaveImage") }));
        assert!(obj.values().any(|node| {
            node.get("class_type").and_then(Value::as_str) == Some("KSamplerAdvanced")
        }));
    }

    #[test]
    fn test_apply_image_text_requires_load_image() {
        let path = project_root().join("assets/workflows/sdxl.json");
        if !path.exists() {
            return;
        }
        let error = load_comfyui_prompt_with_overrides(
            path.to_string_lossy().as_ref(),
            ComfyPromptOverrides {
                prompt: Some("a cat"),
                negative_prompt: Some("text"),
                input_image: Some("input.png"),
                denoise: Some(0.5),
            },
        )
        .unwrap_err();
        assert!(error.contains("LoadImage"));
    }
}
