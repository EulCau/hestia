use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

const PROCESS_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

fn wait_child_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if started.elapsed() >= timeout {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Information about a discovered local model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub manufacturer: String,
    pub model_name: String,
    pub file_path: String,
    pub size_bytes: Option<u64>,
}

pub fn list_available_models(models_dir: &str) -> Vec<ModelInfo> {
    let base = PathBuf::from(models_dir);
    if !base.is_dir() {
        return vec![];
    }
    let mut models: Vec<ModelInfo> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let manufacturer = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            if let Ok(model_files) = std::fs::read_dir(&path) {
                for mf in model_files.flatten() {
                    let mp = mf.path();
                    if mp.extension().map_or(false, |e| e == "gguf") {
                        let model_name = mp
                            .file_stem()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let size_bytes = std::fs::metadata(&mp).ok().map(|m| m.len());
                        models.push(ModelInfo {
                            manufacturer: manufacturer.clone(),
                            model_name,
                            file_path: mp.to_string_lossy().to_string(),
                            size_bytes,
                        });
                    }
                }
            }
        }
    }
    models.sort_by(|a, b| {
        a.manufacturer
            .cmp(&b.manufacturer)
            .then_with(|| a.model_name.cmp(&b.model_name))
    });
    info!(models_dir = %models_dir, count = models.len(), "scanned models directory");
    models
}

pub fn resolve_model_path(models_dir: &str, manufacturer: &str, model_name: &str) -> PathBuf {
    PathBuf::from(models_dir)
        .join(manufacturer)
        .join(format!("{}.gguf", model_name))
}

pub fn find_model_path(models_dir: &str, model_spec: &str) -> Option<PathBuf> {
    let abs = PathBuf::from(model_spec);
    if abs.is_absolute() && abs.exists() && abs.extension().map_or(false, |e| e == "gguf") {
        return Some(abs);
    }
    if let Some((manufacturer, model_name)) = model_spec.split_once('/') {
        let path = resolve_model_path(models_dir, manufacturer, model_name);
        if path.exists() {
            return Some(path);
        }
    }
    for model in list_available_models(models_dir) {
        if model.model_name == model_spec
            || model.file_path.ends_with(&format!("/{}.gguf", model_spec))
        {
            return Some(PathBuf::from(&model.file_path));
        }
    }
    warn!(model_spec = %model_spec, models_dir = %models_dir, "model not found in models directory");
    None
}

pub fn build_default_load_command(
    backend: &str,
    model_path: &str,
    port: u16,
    host: &str,
) -> String {
    match backend {
        "llama_cpp" => format!(
            "llama-server -m {model_path} --port {port} --host {host} --ctx-size 4096 --n-gpu-layers 999 --flash-attn on --reasoning off"
        ),
        "ollama" => {
            let model_name = std::path::Path::new(model_path)
                .file_stem().and_then(|n| n.to_str()).unwrap_or(model_path);
            format!("ollama pull {model_name}")
        }
        "vllm" => {
            warn!("auto-load is not supported for vLLM; please start it manually");
            String::new()
        }
        other => {
            warn!(backend = %other, "unknown backend, no default load command");
            String::new()
        }
    }
}

pub fn expand_command_placeholders(
    template: &str,
    model_path: &str,
    port: u16,
    host: &str,
) -> String {
    template
        .replace("{model_path}", model_path)
        .replace("{port}", &port.to_string())
        .replace("{host}", host)
}

/// Kill any process listening on the given TCP port via `fuser -k`.
pub fn kill_port(port: u16) {
    let port_arg = format!("{}/tcp", port);
    match Command::new("fuser").args(["-k", &port_arg]).output() {
        Ok(out) => {
            if out.status.success() {
                info!(port, "killed previous process on port");
            }
        }
        Err(e) => {
            warn!(port, error = %e, "fuser not available, cannot auto-free port");
        }
    }
}

/// Manages a backend subprocess lifecycle.
/// On Unix the child is placed in its own process group so that
/// killing the group leader cleans up the entire process tree.
pub struct BackendProcess {
    child: Option<Child>,
    unload_command: Option<String>,
}

impl BackendProcess {
    pub fn new() -> Self {
        Self {
            child: None,
            unload_command: None,
        }
    }

    pub fn set_unload_command(&mut self, command: String) {
        self.unload_command = if command.trim().is_empty() {
            None
        } else {
            Some(command)
        };
    }

    pub fn spawn(&mut self, command: &str, model_label: &str, backend: &str) -> bool {
        self.spawn_inner(command, model_label, backend, None)
    }

    pub fn spawn_in_dir(
        &mut self,
        command: &str,
        working_dir: &Path,
        model_label: &str,
        backend: &str,
    ) -> bool {
        self.spawn_inner(command, model_label, backend, Some(working_dir))
    }

    fn spawn_inner(
        &mut self,
        command: &str,
        model_label: &str,
        backend: &str,
        working_dir: Option<&Path>,
    ) -> bool {
        if command.is_empty() {
            info!(model = %model_label, backend = %backend, "auto-load skipped (empty command)");
            return true;
        }
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return true;
        }

        let program = parts[0];
        let args = &parts[1..];
        let mut cmd = Command::new(program);
        cmd.args(args);
        if let Some(working_dir) = working_dir {
            cmd.current_dir(working_dir);
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                info!(
                    model = %model_label, backend = %backend, program = %program,
                    pid, command = %command, "model loaded successfully"
                );
                self.child = Some(child);
                true
            }
            Err(e) => {
                error!(
                    model = %model_label, backend = %backend, program = %program,
                    command = %command, error = %e, "failed to load model"
                );
                false
            }
        }
    }

    pub fn kill(&mut self, backend: &str) {
        self.kill_impl(backend, false);
    }

    fn kill_silent(&mut self) {
        self.kill_impl("", true);
    }

    fn kill_impl(&mut self, backend: &str, silent: bool) {
        let Some(ref mut child) = self.child else {
            return;
        };
        let pid = child.id();

        if let Some(command) = &self.unload_command {
            let parts: Vec<&str> = command.split_whitespace().collect();
            if let Some((program, args)) = parts.split_first() {
                match Command::new(program).args(args).spawn() {
                    Ok(mut unload_child) => {
                        match wait_child_with_timeout(&mut unload_child, PROCESS_SHUTDOWN_TIMEOUT) {
                            Ok(Some(status)) if status.success() => {
                                info!(backend = %backend, command = %command, "custom unload command succeeded");
                            }
                            Ok(Some(status)) => {
                                warn!(
                                    backend = %backend,
                                    command = %command,
                                    status = %status,
                                    "custom unload command failed, falling back to process termination"
                                );
                            }
                            Ok(None) => {
                                let _ = unload_child.kill();
                                let _ = unload_child.wait();
                                warn!(
                                    backend = %backend,
                                    command = %command,
                                    "custom unload command timed out, falling back to process termination"
                                );
                            }
                            Err(e) => {
                                warn!(
                                    backend = %backend,
                                    command = %command,
                                    error = %e,
                                    "custom unload command wait failed, falling back to process termination"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            backend = %backend,
                            command = %command,
                            error = %e,
                            "custom unload command could not start, falling back to process termination"
                        );
                    }
                }
            }
        }

        // SIGTERM the entire process group first (graceful shutdown)
        #[cfg(unix)]
        unsafe {
            libc::kill(-(pid as i32), libc::SIGTERM);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));

        #[cfg(windows)]
        {
            match Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .status()
            {
                Ok(status) if status.success() => {
                    info!(backend = %backend, pid, "backend process tree killed");
                }
                Ok(status) => {
                    warn!(
                        backend = %backend,
                        pid,
                        status = %status,
                        "taskkill failed, falling back to direct process termination"
                    );
                }
                Err(e) => {
                    warn!(
                        backend = %backend,
                        pid,
                        error = %e,
                        "taskkill could not start, falling back to direct process termination"
                    );
                }
            }
        }

        match child.kill() {
            Ok(()) => {
                if silent {
                    info!(pid, "backend process killed (drop)");
                } else {
                    info!(backend = %backend, pid, "model unloaded successfully");
                }
            }
            Err(e) => {
                // InvalidInput means process already exited — that's fine
                if e.kind() != std::io::ErrorKind::InvalidInput {
                    if silent {
                        warn!(pid, error = %e, "failed to kill backend process (drop)");
                    } else {
                        warn!(backend = %backend, pid, error = %e, "failed to unload model");
                    }
                } else if !silent {
                    info!(backend = %backend, pid, "model unloaded (already exited)");
                }
            }
        }
        match wait_child_with_timeout(child, PROCESS_SHUTDOWN_TIMEOUT) {
            Ok(Some(_)) => {}
            Ok(None) => {
                warn!(backend = %backend, pid, "backend process wait timed out");
            }
            Err(e) => {
                warn!(backend = %backend, pid, error = %e, "failed to wait for backend process");
            }
        }
        self.child = None;
    }

    #[allow(dead_code)]
    pub fn is_running(&mut self) -> bool {
        let Some(child) = self.child.as_mut() else {
            return false;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                info!(pid = child.id(), status = %status, "backend process exited");
                self.child = None;
                false
            }
            Ok(None) => true,
            Err(error) => {
                warn!(pid = child.id(), error = %error, "failed to poll backend process");
                true
            }
        }
    }
}

impl Drop for BackendProcess {
    fn drop(&mut self) {
        self.kill_silent();
    }
}
