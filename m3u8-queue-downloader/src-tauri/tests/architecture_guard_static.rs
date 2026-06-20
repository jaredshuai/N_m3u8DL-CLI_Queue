//! Static architecture guard tests.
//!
//! Fine-grained architectural contracts enforced by compile-time include_str! scanning.
//! Static counterpart to architecture_guard.rs (runtime fs::read_dir layer rules).
//! Moved out of src/lib.rs; these guards have zero runtime coupling to lib internals.

use std::collections::BTreeSet;

fn domain_sources() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "domain/history.rs",
            include_str!("../src/domain/history.rs"),
        ),
        ("domain/mod.rs", include_str!("../src/domain/mod.rs")),
        ("domain/queue.rs", include_str!("../src/domain/queue.rs")),
        (
            "domain/retry_policy.rs",
            include_str!("../src/domain/retry_policy.rs"),
        ),
        ("domain/task.rs", include_str!("../src/domain/task.rs")),
        (
            "domain/artifact.rs",
            include_str!("../src/domain/artifact.rs"),
        ),
        (
            "domain/run_session.rs",
            include_str!("../src/domain/run_session.rs"),
        ),
        (
            "domain/terminal.rs",
            include_str!("../src/domain/terminal.rs"),
        ),
    ]
}

fn declared_domain_module_paths() -> BTreeSet<String> {
    let mut modules = include_str!("../src/domain/mod.rs")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub(crate) mod "))
        .filter_map(|rest| rest.strip_suffix(';'))
        .map(|module| format!("domain/{module}.rs"))
        .collect::<BTreeSet<_>>();
    modules.insert("domain/mod.rs".to_string());
    modules
}

fn application_sources() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "application/app_error.rs",
            include_str!("../src/application/app_error.rs"),
        ),
        (
            "application/close_policy.rs",
            include_str!("../src/application/close_policy.rs"),
        ),
        (
            "application/download_directory_orchestrator.rs",
            include_str!("../src/application/download_directory_orchestrator.rs"),
        ),
        (
            "application/download_directory.rs",
            include_str!("../src/application/download_directory.rs"),
        ),
        (
            "application/exit_orchestrator.rs",
            include_str!("../src/application/exit_orchestrator.rs"),
        ),
        (
            "application/exit_use_cases.rs",
            include_str!("../src/application/exit_use_cases.rs"),
        ),
        (
            "application/history_repository_outcomes.rs",
            include_str!("../src/application/history_repository_outcomes.rs"),
        ),
        (
            "application/history_orchestrator.rs",
            include_str!("../src/application/history_orchestrator.rs"),
        ),
        (
            "application/history_use_cases.rs",
            include_str!("../src/application/history_use_cases.rs"),
        ),
        (
            "application/history_task_page.rs",
            include_str!("../src/application/history_task_page.rs"),
        ),
        (
            "application/mod.rs",
            include_str!("../src/application/mod.rs"),
        ),
        (
            "application/process_runner_outcomes.rs",
            include_str!("../src/application/process_runner_outcomes.rs"),
        ),
        (
            "application/query_models.rs",
            include_str!("../src/application/query_models.rs"),
        ),
        (
            "application/queue_mutation_orchestrator.rs",
            include_str!("../src/application/queue_mutation_orchestrator.rs"),
        ),
        (
            "application/queue_query_orchestrator.rs",
            include_str!("../src/application/queue_query_orchestrator.rs"),
        ),
        (
            "application/queue_repository_outcomes.rs",
            include_str!("../src/application/queue_repository_outcomes.rs"),
        ),
        (
            "application/queue_requests.rs",
            include_str!("../src/application/queue_requests.rs"),
        ),
        (
            "application/queue_scheduler_outcomes.rs",
            include_str!("../src/application/queue_scheduler_outcomes.rs"),
        ),
        (
            "application/queue_scheduling_orchestrator.rs",
            include_str!("../src/application/queue_scheduling_orchestrator.rs"),
        ),
        (
            "application/queue_start_orchestrator.rs",
            include_str!("../src/application/queue_start_orchestrator.rs"),
        ),
        (
            "application/queue_state_snapshot.rs",
            include_str!("../src/application/queue_state_snapshot.rs"),
        ),
        (
            "application/run_completion_orchestrator.rs",
            include_str!("../src/application/run_completion_orchestrator.rs"),
        ),
        (
            "application/settings.rs",
            include_str!("../src/application/settings.rs"),
        ),
        (
            "application/settings_orchestrator.rs",
            include_str!("../src/application/settings_orchestrator.rs"),
        ),
        (
            "application/settings_query_orchestrator.rs",
            include_str!("../src/application/settings_query_orchestrator.rs"),
        ),
        (
            "application/shutdown_scheduler_outcomes.rs",
            include_str!("../src/application/shutdown_scheduler_outcomes.rs"),
        ),
        (
            "application/task_creation_orchestrator.rs",
            include_str!("../src/application/task_creation_orchestrator.rs"),
        ),
        (
            "application/task_lifecycle_orchestrator.rs",
            include_str!("../src/application/task_lifecycle_orchestrator.rs"),
        ),
        (
            "application/task_output_event_orchestrator.rs",
            include_str!("../src/application/task_output_event_orchestrator.rs"),
        ),
        (
            "application/task_process_events.rs",
            include_str!("../src/application/task_process_events.rs"),
        ),
        (
            "application/task_process_start_request.rs",
            include_str!("../src/application/task_process_start_request.rs"),
        ),
        (
            "application/task_snapshot.rs",
            include_str!("../src/application/task_snapshot.rs"),
        ),
        (
            "application/terminal_history_orchestrator.rs",
            include_str!("../src/application/terminal_history_orchestrator.rs"),
        ),
        (
            "application/terminal_history_use_cases.rs",
            include_str!("../src/application/terminal_history_use_cases.rs"),
        ),
        (
            "application/terminal_output_outcomes.rs",
            include_str!("../src/application/terminal_output_outcomes.rs"),
        ),
        (
            "application/terminal_output_page.rs",
            include_str!("../src/application/terminal_output_page.rs"),
        ),
        (
            "application/terminal_output_orchestrator.rs",
            include_str!("../src/application/terminal_output_orchestrator.rs"),
        ),
        (
            "application/terminal_output_use_cases.rs",
            include_str!("../src/application/terminal_output_use_cases.rs"),
        ),
        (
            "application/task_runtime_state.rs",
            include_str!("../src/application/task_runtime_state.rs"),
        ),
    ]
}

fn declared_application_module_paths() -> BTreeSet<String> {
    let mut modules = include_str!("../src/application/mod.rs")
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("pub(crate) mod ")
                .and_then(|rest| rest.strip_suffix(';'))
                .map(|module| format!("application/{module}.rs"))
        })
        .collect::<BTreeSet<_>>();
    modules.insert("application/mod.rs".to_string());
    modules
}

fn port_sources() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "ports/application_control.rs",
            include_str!("../src/ports/application_control.rs"),
        ),
        ("ports/clock.rs", include_str!("../src/ports/clock.rs")),
        (
            "ports/diagnostics.rs",
            include_str!("../src/ports/diagnostics.rs"),
        ),
        (
            "ports/directory_opener.rs",
            include_str!("../src/ports/directory_opener.rs"),
        ),
        (
            "ports/download_directory_resolver.rs",
            include_str!("../src/ports/download_directory_resolver.rs"),
        ),
        (
            "ports/event_publisher.rs",
            include_str!("../src/ports/event_publisher.rs"),
        ),
        (
            "ports/history_repository.rs",
            include_str!("../src/ports/history_repository.rs"),
        ),
        ("ports/mod.rs", include_str!("../src/ports/mod.rs")),
        (
            "ports/process_runner.rs",
            include_str!("../src/ports/process_runner.rs"),
        ),
        (
            "ports/queue_repository.rs",
            include_str!("../src/ports/queue_repository.rs"),
        ),
        (
            "ports/settings_repository.rs",
            include_str!("../src/ports/settings_repository.rs"),
        ),
        (
            "ports/shutdown_scheduler.rs",
            include_str!("../src/ports/shutdown_scheduler.rs"),
        ),
        (
            "ports/task_id_generator.rs",
            include_str!("../src/ports/task_id_generator.rs"),
        ),
        (
            "ports/terminal_output_repository.rs",
            include_str!("../src/ports/terminal_output_repository.rs"),
        ),
    ]
}

fn declared_port_module_paths() -> BTreeSet<String> {
    let mut modules = include_str!("../src/ports/mod.rs")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub(crate) mod "))
        .filter_map(|rest| rest.strip_suffix(';'))
        .map(|module| format!("ports/{module}.rs"))
        .collect::<BTreeSet<_>>();
    modules.insert("ports/mod.rs".to_string());
    modules
}

fn assert_sources_do_not_contain(
    sources: Vec<(&'static str, &'static str)>,
    forbidden_patterns: &[&str],
    layer_name: &str,
) {
    for (name, source) in sources {
        for forbidden in forbidden_patterns {
            assert!(
                !source.contains(forbidden),
                "{layer_name} source {name} should not contain outward dependency {forbidden}"
            );
        }
    }
}

#[test]
fn main_window_capability_does_not_allow_renderer_event_emit() {
    let capability: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/default.json"))
            .expect("capability json");
    let permissions = capability["permissions"]
        .as_array()
        .expect("permissions array")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();

    assert!(
        !permissions.contains(&"core:event:allow-emit"),
        "renderer event emit can forge backend lifecycle events"
    );
    assert!(
        !permissions.contains(&"core:event:allow-emit-to"),
        "renderer event emit-to can spoof frontend display events"
    );
    assert!(
        !permissions.contains(&"core:event:default"),
        "core:event:default expands to renderer event emit permissions"
    );
    assert!(
        !permissions.contains(&"core:default"),
        "core:default includes core:event:default"
    );
    assert!(permissions.contains(&"core:event:deny-emit"));
    assert!(permissions.contains(&"core:event:deny-emit-to"));
}

#[test]
fn domain_layer_has_no_outward_dependencies() {
    assert_sources_do_not_contain(
        domain_sources(),
        &[
            "crate::adapters",
            "crate::application",
            "crate::ports",
            "tauri::",
            "tokio::",
            "std::fs",
            "std::path",
            "std::process",
            "serde",
            "Serialize",
            "Deserialize",
            "#[serde",
            "serde_json",
            "use uuid::",
            "Uuid::",
            "Utc::now",
        ],
        "domain",
    );
}

#[test]
fn domain_layer_guard_covers_every_declared_module() {
    let covered = domain_sources()
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        covered,
        declared_domain_module_paths(),
        "domain_sources() must cover every module declared in domain/mod.rs"
    );
}

#[test]
fn application_layer_guard_covers_every_declared_module() {
    let covered = application_sources()
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        covered,
        declared_application_module_paths(),
        "application_sources() must cover every module declared in application/mod.rs"
    );
}

#[test]
fn application_layer_has_no_adapter_or_framework_dependencies() {
    assert_sources_do_not_contain(
        application_sources(),
        &[
            "crate::adapters",
            "tauri::",
            "tokio::process",
            "std::fs",
            "std::path",
            "std::process::Command",
            "serde",
            "Serialize",
            "Deserialize",
            "#[serde",
            "serde_json",
        ],
        "application",
    );
}

#[test]
fn application_layer_uses_diagnostics_port_for_logging() {
    assert_sources_do_not_contain(
        application_sources(),
        &["eprintln!", "println!"],
        "application",
    );

    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    assert!(
        dependency_graph_source.contains("ports::diagnostics::Diagnostics"),
        "runtime logging should be routed through a diagnostics port"
    );
    assert!(
        dependency_graph_source.contains("Arc<dyn Diagnostics>"),
        "DependencyGraph should hold diagnostics as a trait object"
    );
}

#[test]
fn runtime_adapters_route_warnings_through_diagnostics_port() {
    let pending_history_worker_source =
        include_str!("../src/composition/pending_history_worker.rs");
    let commands_source = include_str!("../src/composition/tauri_commands.rs");
    let tray_source = include_str!("../src/composition/tray.rs");
    let composition_mod_source = include_str!("../src/composition/mod.rs");
    let diagnostics_facade_source = include_str!("../src/composition/diagnostics_facade.rs");

    assert!(
        composition_mod_source.contains("mod diagnostics_facade"),
        "composition should expose diagnostics through a facade instead of DependencyGraph fields"
    );
    assert!(
        diagnostics_facade_source.contains("self.dependencies.diagnostics.warn"),
        "diagnostics facade should own diagnostics port access for adapter warnings"
    );

    for (name, source) in [
        ("pending_history_worker.rs", pending_history_worker_source),
        ("tauri_commands.rs", commands_source),
        ("tray.rs", tray_source),
    ] {
        for forbidden in ["eprintln!", "println!", ".diagnostics", "diagnostics.warn"] {
            assert!(
                !source.contains(forbidden),
                "{name} should route adapter warning {forbidden} through DiagnosticsFacade"
            );
        }
    }
    assert!(pending_history_worker_source.contains("DiagnosticsFacade::new"));
    assert!(tray_source.contains("DiagnosticsFacade::new"));
}

#[test]
fn ports_layer_has_no_adapter_or_framework_dependencies() {
    assert_sources_do_not_contain(
        port_sources(),
        &[
            "crate::adapters",
            "tauri::",
            "tokio::process",
            "std::fs",
            "std::path",
            "std::process::Command",
            "std::io::Error",
            "serde_json",
        ],
        "ports",
    );
}

#[test]
fn ports_layer_guard_covers_every_declared_module() {
    let covered = port_sources()
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        covered,
        declared_port_module_paths(),
        "port_sources() must cover every module declared in ports/mod.rs"
    );
}

#[test]
fn ports_layer_does_not_own_application_outcome_models() {
    for (name, source) in port_sources() {
        for forbidden in ["pub(crate) enum", "pub enum", "\nenum "] {
            assert!(
                !source.contains(forbidden),
                "ports source {name} should describe capabilities, not own outcome model {forbidden}"
            );
        }
    }
}

#[test]
fn app_bootstrap_is_composition_root_for_adapter_construction() {
    let lib_runtime_source = include_str!("../src/lib.rs")
        .split("mod tests {")
        .next()
        .expect("lib runtime source");
    let bootstrap_source = include_str!("../src/composition/app_bootstrap.rs");

    for adapter_constructor in [
        "CliOutputStore::new",
        "HistoryStore::new",
        "QueueManager::new",
        "SettingsStore::new",
        "ShutdownManager::new",
        "TauriTaskProcessRunnerFactory::new",
        "TauriApplicationControl::new",
        "UuidTaskIdGenerator",
        "SystemClock",
        "StderrDiagnostics",
    ] {
        assert!(
            !lib_runtime_source.contains(adapter_constructor),
            "lib.rs should delegate adapter construction to composition bootstrap"
        );
        assert!(
            bootstrap_source.contains(adapter_constructor),
            "composition bootstrap should own adapter construction {adapter_constructor}"
        );
    }

    assert!(lib_runtime_source.contains(".setup(app_bootstrap::setup_app)"));
}

#[test]
fn dependency_graph_is_owned_by_composition_layer() {
    let application_mod_source = include_str!("../src/application/mod.rs");
    let composition_mod_source = include_str!("../src/composition/mod.rs");
    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    let download_directory_command_facade_source =
        include_str!("../src/composition/download_directory_command_facade.rs");
    let exit_command_facade_source = include_str!("../src/composition/exit_command_facade.rs");
    let history_command_facade_source =
        include_str!("../src/composition/history_command_facade.rs");
    let pending_history_facade_source =
        include_str!("../src/composition/pending_history_facade.rs");
    let queue_command_facade_source = include_str!("../src/composition/queue_command_facade.rs");
    let read_model_query_facade_source =
        include_str!("../src/composition/read_model_query_facade.rs");
    let runtime_facade_source = include_str!("../src/composition/runtime_facade.rs");
    let settings_command_facade_source =
        include_str!("../src/composition/settings_command_facade.rs");

    assert!(
        !application_mod_source.contains("mod app_state"),
        "application layer should not own the runtime dependency graph"
    );
    assert!(
        composition_mod_source.contains("mod dependency_graph"),
        "composition layer should own the runtime dependency graph module"
    );
    assert!(
        dependency_graph_source.contains("pub struct DependencyGraph"),
        "composition dependency graph should expose the Tauri-managed dependency graph"
    );
    assert!(
        !dependency_graph_source.contains("AppState"),
        "composition dependency graph should not be named like a framework state bag"
    );
    for dependency_field in [
        "terminal_output_repository",
        "history_repository",
        "queue_repository",
        "settings_repository",
        "download_directory_resolver",
        "directory_opener",
        "application_control",
        "shutdown_scheduler",
        "task_process_supervisor",
        "task_process_runner_factory",
        "task_id_generator",
        "clock",
        "diagnostics",
    ] {
        assert!(
            dependency_graph_source
                .contains(&format!("pub(in crate::composition) {dependency_field}")),
            "DependencyGraph field {dependency_field} should only be visible inside composition"
        );
    }
    assert!(
        !dependency_graph_source.contains("pub(crate) queue_repository")
            && !dependency_graph_source.contains("pub(crate) diagnostics"),
        "DependencyGraph fields should not be crate-visible to adapters"
    );
    assert!(
        !dependency_graph_source.contains("UseCases::new"),
        "dependency graph should hold dependencies, not construct adapter-facing use cases"
    );
    assert!(
        dependency_graph_source.contains("fn queue_scheduling_orchestrator")
            && dependency_graph_source.contains("QueueSchedulingPorts::new"),
        "dependency graph should centralize queue scheduling port wiring"
    );
    assert!(
        dependency_graph_source.contains("fn queue_mutation_orchestrator")
            && dependency_graph_source.contains("QueueMutationPorts::new")
            && queue_command_facade_source.contains("queue_mutation_orchestrator")
            && !queue_command_facade_source.contains("queue_repository.as_ref()"),
        "dependency graph should centralize queue mutation port wiring"
    );
    assert!(
        dependency_graph_source.contains("fn queue_start_orchestrator")
            && dependency_graph_source.contains("QueueStartPorts::new")
            && queue_command_facade_source.contains("queue_start_orchestrator")
            && !queue_command_facade_source.contains("shutdown_scheduler.as_ref()"),
        "dependency graph should centralize queue start port wiring"
    );
    assert!(
        dependency_graph_source.contains("fn queue_query_orchestrator")
            && dependency_graph_source.contains("QueueQueryPorts::new"),
        "dependency graph should centralize queue query port wiring"
    );
    assert!(
        dependency_graph_source.contains("fn task_output_event_orchestrator")
            && dependency_graph_source.contains("TaskOutputEventPorts::new")
            && !runtime_facade_source.contains("TaskOutputEventPorts::new"),
        "dependency graph should centralize task output event port wiring"
    );
    assert!(
        dependency_graph_source.contains("fn terminal_output_orchestrator")
            && dependency_graph_source.contains("TerminalOutputPorts::new"),
        "dependency graph should centralize terminal output query port wiring"
    );
    assert!(
        dependency_graph_source.contains("fn task_lifecycle_orchestrator")
            && dependency_graph_source.contains("TaskLifecyclePorts::new")
            && !runtime_facade_source.contains("TaskLifecyclePorts::new")
            && !runtime_facade_source.contains("terminal_output_repository.as_ref()")
            && !runtime_facade_source.contains("shutdown_scheduler.as_ref()"),
        "dependency graph should centralize task lifecycle port wiring"
    );
    assert!(
        dependency_graph_source.contains("TaskLifecyclePorts::new(scheduling_ports)")
            && !dependency_graph_source.contains(
                "TaskLifecyclePorts::new(scheduling_ports, self.shutdown_scheduler.as_ref())"
            ),
        "TaskLifecyclePorts wiring should not receive ShutdownScheduler directly; QueueSchedulingPorts owns terminal failure marking"
    );
    assert!(
        dependency_graph_source.contains("fn settings_orchestrator")
            && dependency_graph_source.contains("SettingsPorts::new")
            && !settings_command_facade_source.contains("SettingsPorts::new"),
        "dependency graph should centralize settings port wiring"
    );
    assert!(
        dependency_graph_source.contains("fn settings_query_orchestrator")
            && dependency_graph_source.contains("SettingsQueryPorts::new"),
        "dependency graph should centralize settings query port wiring"
    );
    assert!(
        dependency_graph_source.contains("fn exit_orchestrator")
            && dependency_graph_source.contains("ExitPorts::new")
            && !exit_command_facade_source.contains("ExitPorts::new"),
        "dependency graph should centralize exit port wiring"
    );
    for facade_source in [&queue_command_facade_source, &runtime_facade_source] {
        assert!(
            !facade_source.contains("QueueSchedulingPorts::new"),
            "facades should request queue scheduling ports from DependencyGraph instead of duplicating wiring"
        );
    }
    assert!(
        queue_command_facade_source.contains("handle_queue_add")
            && queue_command_facade_source.contains("handle_task_removal")
            && queue_command_facade_source.contains("handle_tasks_reorder")
            && queue_command_facade_source.contains("handle_queue_retry")
            && queue_command_facade_source.contains("handle_queue_start")
            && queue_command_facade_source.contains("handle_queue_pause"),
        "queue command facade should wire split queue use cases directly"
    );
    assert!(
        dependency_graph_source.contains("fn task_creation_orchestrator")
            && dependency_graph_source.contains("TaskCreationPorts::new")
            && queue_command_facade_source.contains("task_creation_orchestrator")
            && !queue_command_facade_source.contains("task_id_generator.as_ref()")
            && !queue_command_facade_source.contains("clock.as_ref()"),
        "dependency graph should centralize task creation port wiring"
    );
    assert!(
        download_directory_command_facade_source.contains("ports.open_download_dir"),
        "download directory command facade should own adapter-facing download directory port wiring"
    );
    assert!(
        dependency_graph_source.contains("fn download_directory_orchestrator")
            && dependency_graph_source.contains("DownloadDirectoryPorts::new")
            && !download_directory_command_facade_source.contains("DownloadDirectoryPorts::new")
            && !download_directory_command_facade_source.contains("settings_repository.as_ref()"),
        "dependency graph should centralize download directory port wiring"
    );
    assert!(
        exit_command_facade_source.contains("ExitUseCases::new"),
        "exit command facade should own adapter-facing exit use-case wiring"
    );
    assert!(
        history_command_facade_source.contains("HistoryUseCases::new"),
        "history command facade should own adapter-facing history command wiring"
    );
    assert!(
        dependency_graph_source.contains("fn history_orchestrator")
            && dependency_graph_source.contains("HistoryPorts::new")
            && !dependency_graph_source.contains("fn history_repository_handle")
            && !history_command_facade_source.contains("history_repository.as_ref()"),
        "dependency graph should centralize history command port wiring"
    );
    assert!(
        pending_history_facade_source
            .contains("terminal_history_use_cases::flush_pending_history_tasks"),
        "pending history facade should own pending terminal-history flush wiring"
    );
    assert!(
        read_model_query_facade_source.contains("queue_query_orchestrator")
            && read_model_query_facade_source.contains("settings_query_orchestrator")
            && read_model_query_facade_source.contains("HistoryUseCases::new")
            && read_model_query_facade_source.contains("TerminalOutputUseCases::new"),
        "read model query facade should own adapter-facing query wiring"
    );
    assert!(
        runtime_facade_source.contains("TaskLifecyclePorts")
            && runtime_facade_source.contains("TaskOutputEventPorts")
            && runtime_facade_source.contains("task_lifecycle_orchestrator")
            && runtime_facade_source.contains("task_output_event_orchestrator"),
        "runtime facade should own adapter-facing runtime port wiring"
    );
    assert!(
        settings_command_facade_source.contains("update_settings_and_handle_auto_action_change"),
        "settings command facade should own adapter-facing settings port wiring"
    );
}

#[test]
fn command_and_tray_adapters_delegate_queue_commands_to_queue_command_facade() {
    let commands_source = include_str!("../src/composition/tauri_commands.rs");
    let tray_source = include_str!("../src/composition/tray.rs");
    let composition_mod_source = include_str!("../src/composition/mod.rs");

    // Keep UI adapters thin: queue mutation, scheduling, and close routing belong
    // behind queue use cases so policy does not drift across adapters.
    for forbidden in [
        "create_process_runner",
        "process_runner.as_ref()",
        "ExitUseCases::new",
        "HistoryUseCases::new",
        "TerminalOutputUseCases::new",
        "terminal_history_use_cases::flush_pending_history_tasks",
        "history_repository.clone",
        "terminal_output_repository.clone",
    ] {
        assert!(
            !commands_source.contains(forbidden),
            "commands.rs should delegate {forbidden} through queue use cases"
        );
    }

    for forbidden in [
        "create_process_runner",
        "process_runner.as_ref()",
        "ExitUseCases::new",
    ] {
        assert!(
            !tray_source.contains(forbidden),
            "tray.rs should delegate {forbidden} through queue use cases"
        );
    }

    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    assert!(
        composition_mod_source.contains("mod queue_command_facade"),
        "composition should expose a queue command facade so DependencyGraph does not own adapter-facing queue commands"
    );
    assert!(
        commands_source.contains("QueueCommandFacade::new"),
        "commands.rs should route queue mutations through QueueCommandFacade"
    );
    assert!(
        tray_source.contains("QueueCommandFacade::new"),
        "tray.rs should route queue mutations through QueueCommandFacade"
    );
    let queue_command_facade_source = include_str!("../src/composition/queue_command_facade.rs");
    assert!(queue_command_facade_source.contains("handle_queue_add"));
    assert!(queue_command_facade_source.contains("handle_task_removal"));
    assert!(queue_command_facade_source.contains("handle_tasks_reorder"));
    assert!(queue_command_facade_source.contains("handle_queue_retry"));
    assert!(queue_command_facade_source.contains("handle_queue_start"));
    assert!(queue_command_facade_source.contains("handle_queue_pause"));
    assert!(
        queue_command_facade_source.contains("queue_scheduling_orchestrator")
            && dependency_graph_source.contains("QueueSchedulingPorts::new"),
        "queue command facade should obtain scheduling dependencies through the centralized DependencyGraph port bundle"
    );
    assert!(
        queue_command_facade_source.contains("queue_mutation_orchestrator")
            && dependency_graph_source.contains("QueueMutationPorts::new"),
        "queue command facade should obtain mutation dependencies through the centralized DependencyGraph port bundle"
    );
    assert!(
        queue_command_facade_source.contains("task_creation_orchestrator")
            && dependency_graph_source.contains("TaskCreationPorts::new"),
        "queue command facade should obtain task creation dependencies through the centralized DependencyGraph port bundle"
    );
    assert!(
        queue_command_facade_source.contains("queue_start_orchestrator")
            && dependency_graph_source.contains("QueueStartPorts::new"),
        "queue command facade should obtain start dependencies through the centralized DependencyGraph port bundle"
    );
    assert!(
        !dependency_graph_source.contains("pub(crate) async fn add_task"),
        "DependencyGraph should only provide queue wiring, not adapter-facing add_task"
    );
    assert!(
        !dependency_graph_source.contains("pub(crate) async fn remove_task"),
        "DependencyGraph should only provide queue wiring, not adapter-facing remove_task"
    );
    assert!(
        !dependency_graph_source.contains("pub(crate) async fn retry_task"),
        "DependencyGraph should only provide queue wiring, not adapter-facing retry_task"
    );
    assert!(
        !dependency_graph_source.contains("pub(crate) async fn reorder_tasks"),
        "DependencyGraph should only provide queue wiring, not adapter-facing reorder_tasks"
    );
    assert!(
        !dependency_graph_source.contains("pub(crate) async fn start_queue"),
        "DependencyGraph should only provide queue wiring, not adapter-facing start_queue"
    );
    assert!(
        !dependency_graph_source.contains("pub(crate) async fn pause_queue"),
        "DependencyGraph should only provide queue wiring, not adapter-facing pause_queue"
    );
    assert!(queue_command_facade_source.contains("create_task_process_runner"));
}

#[test]
fn tauri_commands_delegate_read_model_queries_to_read_model_query_facade() {
    let commands_source = include_str!("../src/composition/tauri_commands.rs");
    let composition_mod_source = include_str!("../src/composition/mod.rs");
    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    let read_model_query_facade_source =
        include_str!("../src/composition/read_model_query_facade.rs");

    assert!(
        composition_mod_source.contains("mod read_model_query_facade"),
        "composition should expose a read model query facade so DependencyGraph does not own adapter-facing query commands"
    );
    assert!(
        commands_source.contains("ReadModelQueryFacade::new"),
        "commands.rs should route read model queries through ReadModelQueryFacade"
    );

    for forbidden in ["HistoryUseCases::new", "TerminalOutputUseCases::new"] {
        assert!(
            !commands_source.contains(forbidden),
            "commands.rs should delegate {forbidden} through ReadModelQueryFacade"
        );
    }

    for forbidden in [
        "pub(crate) fn history_page_query",
        "pub(crate) fn cli_output_tail_query",
        "pub(crate) fn cli_output_page_query",
        "pub(crate) fn cli_terminal_state_query",
    ] {
        assert!(
            !dependency_graph_source.contains(forbidden),
            "DependencyGraph should only provide dependency wiring, not adapter-facing query method {forbidden}"
        );
    }

    assert!(read_model_query_facade_source.contains("queue_query_orchestrator"));
    assert!(
        read_model_query_facade_source.contains("queue_query_orchestrator")
            && !read_model_query_facade_source.contains("queue_repository.as_ref()"),
        "read model query facade should obtain queue query dependencies through DependencyGraph"
    );
    assert!(read_model_query_facade_source.contains("settings_query_orchestrator"));
    assert!(
        read_model_query_facade_source.contains("settings_query_orchestrator")
            && !read_model_query_facade_source.contains("settings_repository.as_ref()"),
        "read model query facade should obtain settings query dependencies through DependencyGraph"
    );
    assert!(read_model_query_facade_source.contains("HistoryUseCases::new"));
    assert!(
        read_model_query_facade_source.contains("history_orchestrator")
            && !read_model_query_facade_source.contains("history_repository_handle")
            && !read_model_query_facade_source.contains("HistoryPorts::new"),
        "read model query facade should obtain owned history query dependencies through DependencyGraph"
    );
    assert!(read_model_query_facade_source.contains("TerminalOutputUseCases::new"));
    assert!(
        read_model_query_facade_source.contains("terminal_output_orchestrator")
            && !read_model_query_facade_source.contains("terminal_output_repository"),
        "read model query facade should obtain owned terminal output query dependencies through DependencyGraph"
    );
}

#[test]
fn tauri_commands_delegate_history_commands_to_history_command_facade() {
    let commands_source = include_str!("../src/composition/tauri_commands.rs");
    let composition_mod_source = include_str!("../src/composition/mod.rs");
    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    let history_command_facade_source =
        include_str!("../src/composition/history_command_facade.rs");

    assert!(
        composition_mod_source.contains("mod history_command_facade"),
        "composition should expose a history command facade so DependencyGraph does not own adapter-facing history commands"
    );
    assert!(
        commands_source.contains("HistoryCommandFacade::new"),
        "commands.rs should route history commands through HistoryCommandFacade"
    );

    for forbidden in [
        "state.history_use_cases",
        "HistoryUseCases::new",
        "remove_history_task_impl",
    ] {
        assert!(
            !commands_source.contains(forbidden),
            "commands.rs should delegate {forbidden} through HistoryCommandFacade"
        );
    }
    assert!(
        !dependency_graph_source.contains("pub(crate) fn history_use_cases"),
        "DependencyGraph should only expose history dependencies, not an adapter-facing history use-case factory"
    );
    assert!(
        history_command_facade_source.contains("history_orchestrator")
            && dependency_graph_source.contains("HistoryPorts::new"),
        "HistoryCommandFacade should obtain history dependency wiring from DependencyGraph"
    );
    assert!(history_command_facade_source.contains("HistoryUseCases::new"));
}

#[test]
fn command_and_tray_adapters_delegate_exit_commands_to_exit_command_facade() {
    let commands_source = include_str!("../src/composition/tauri_commands.rs");
    let tray_source = include_str!("../src/composition/tray.rs");
    let composition_mod_source = include_str!("../src/composition/mod.rs");
    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        composition_mod_source.contains("mod exit_command_facade"),
        "composition should expose an exit command facade so DependencyGraph does not own adapter-facing close commands"
    );
    assert!(
        commands_source.contains("ExitCommandFacade::new"),
        "commands.rs should route close commands through ExitCommandFacade"
    );
    assert!(
        tray_source.contains("ExitCommandFacade::new"),
        "tray.rs should route close commands through ExitCommandFacade"
    );

    for (name, source) in [("commands.rs", commands_source), ("tray.rs", tray_source)] {
        for forbidden in ["state.exit_use_cases", "ExitUseCases::new"] {
            assert!(
                !source.contains(forbidden),
                "{name} should delegate {forbidden} through ExitCommandFacade"
            );
        }
    }

    assert!(
        !dependency_graph_source.contains("pub(crate) fn exit_use_cases"),
        "DependencyGraph should only expose exit dependencies, not an adapter-facing exit use-case factory"
    );
}

#[test]
fn tauri_commands_delegate_settings_commands_to_settings_command_facade() {
    let commands_source = include_str!("../src/composition/tauri_commands.rs");
    let composition_mod_source = include_str!("../src/composition/mod.rs");
    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    let settings_command_facade_source =
        include_str!("../src/composition/settings_command_facade.rs");

    assert!(
        composition_mod_source.contains("mod settings_command_facade"),
        "composition should expose a settings command facade so DependencyGraph does not own adapter-facing settings commands"
    );
    assert!(
        commands_source.contains("SettingsCommandFacade::new"),
        "commands.rs should route settings mutations through SettingsCommandFacade"
    );

    for forbidden in [".settings_use_cases(", "SettingsUseCases::new"] {
        assert!(
            !commands_source.contains(forbidden),
            "commands.rs should delegate {forbidden} through SettingsCommandFacade"
        );
    }

    assert!(
        !dependency_graph_source.contains("pub(crate) fn settings_use_cases"),
        "DependencyGraph should only expose settings dependencies, not an adapter-facing settings use-case factory"
    );
    assert!(
        settings_command_facade_source.contains("update_settings_and_handle_auto_action_change")
    );
}

#[test]
fn command_and_tray_adapters_delegate_download_directory_commands_to_download_directory_command_facade(
) {
    let commands_source = include_str!("../src/composition/tauri_commands.rs");
    let tray_source = include_str!("../src/composition/tray.rs");
    let composition_mod_source = include_str!("../src/composition/mod.rs");
    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    let download_directory_command_facade_source =
        include_str!("../src/composition/download_directory_command_facade.rs");

    assert!(
        composition_mod_source.contains("mod download_directory_command_facade"),
        "composition should expose a download directory command facade so DependencyGraph does not own adapter-facing directory commands"
    );
    assert!(
        commands_source.contains("DownloadDirectoryCommandFacade::new"),
        "commands.rs should route download directory commands through DownloadDirectoryCommandFacade"
    );
    assert!(
        tray_source.contains("DownloadDirectoryCommandFacade::new"),
        "tray.rs should route download directory commands through DownloadDirectoryCommandFacade"
    );

    for (name, source) in [("commands.rs", commands_source), ("tray.rs", tray_source)] {
        for forbidden in [
            "state.download_directory_use_cases",
            "DownloadDirectoryUseCases::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{name} should delegate {forbidden} through DownloadDirectoryCommandFacade"
            );
        }
    }

    assert!(
        !dependency_graph_source.contains("pub(crate) fn download_directory_use_cases"),
        "DependencyGraph should only expose download directory dependencies, not an adapter-facing directory use-case factory"
    );
    assert!(download_directory_command_facade_source.contains("ports.open_download_dir"));
}

#[test]
fn event_handlers_delegate_runtime_use_cases_to_runtime_facade() {
    let event_handlers_source = include_str!("../src/composition/event_handlers.rs");
    let composition_mod_source = include_str!("../src/composition/mod.rs");
    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    let runtime_facade_source = include_str!("../src/composition/runtime_facade.rs");

    assert!(
        composition_mod_source.contains("mod runtime_facade"),
        "composition should expose a runtime facade so DependencyGraph does not become a service god-object"
    );
    assert!(
        event_handlers_source.contains("RuntimeFacade::new"),
        "event handlers should delegate runtime orchestration through RuntimeFacade"
    );
    assert!(event_handlers_source.contains("handle_task_lifecycle_event"));
    assert!(event_handlers_source.contains("handle_task_output_event"));
    assert!(runtime_facade_source.contains("TaskLifecyclePorts"));
    assert!(runtime_facade_source.contains("TaskOutputEventPorts"));
    assert!(
        !dependency_graph_source.contains("pub(crate) async fn handle_task_lifecycle_event"),
        "DependencyGraph should only provide wiring, not adapter-facing lifecycle orchestration"
    );
    assert!(
        !dependency_graph_source.contains("pub(crate) async fn handle_task_output_event"),
        "DependencyGraph should only provide wiring, not adapter-facing output orchestration"
    );
    assert!(
        !dependency_graph_source.contains("pub(crate) fn task_lifecycle_use_cases"),
        "DependencyGraph should not expose runtime lifecycle use-case factories"
    );
    assert!(
        !dependency_graph_source.contains("pub(crate) fn task_output_event_use_cases"),
        "DependencyGraph should not expose runtime output use-case factories"
    );

    for forbidden in [
        "create_process_runner",
        "TaskLifecycleEvent::Completed",
        "TaskLifecycleEvent::Failed",
        "terminal_output_repository.as_ref()",
        "history_repository.as_ref()",
        "settings_repository.as_ref()",
        "download_directory_resolver.as_ref()",
        "shutdown_scheduler.as_ref()",
    ] {
        assert!(
            !event_handlers_source.contains(forbidden),
            "event_handlers.rs should delegate {forbidden} through RuntimeFacade"
        );
    }
}

#[test]
fn runtime_process_runners_are_created_through_composition_facades() {
    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    let queue_command_facade_source = include_str!("../src/composition/queue_command_facade.rs");
    let runtime_facade_source = include_str!("../src/composition/runtime_facade.rs");
    assert!(
        dependency_graph_source.contains("fn create_task_process_runner")
            && dependency_graph_source.contains("create_process_runner"),
        "DependencyGraph should centralize process runner factory wiring"
    );
    assert!(
        !queue_command_facade_source.contains("task_process_runner_factory")
            && queue_command_facade_source.contains("create_task_process_runner")
    );
    assert!(
        !runtime_facade_source.contains("task_process_runner_factory")
            && runtime_facade_source.contains("create_task_process_runner")
    );

    for (name, source) in [
        (
            "commands.rs",
            include_str!("../src/composition/tauri_commands.rs"),
        ),
        ("tray.rs", include_str!("../src/composition/tray.rs")),
        (
            "event_handlers.rs",
            include_str!("../src/composition/event_handlers.rs"),
        ),
    ] {
        assert!(
            !source.contains("create_process_runner"),
            "{name} should delegate process runner creation to composition facades"
        );
    }

    for (name, source) in [
        (
            "commands.rs",
            include_str!("../src/composition/tauri_commands.rs"),
        ),
        (
            "event_handlers.rs",
            include_str!("../src/composition/event_handlers.rs"),
        ),
        ("tray.rs", include_str!("../src/composition/tray.rs")),
    ] {
        for forbidden in ["TauriTaskProcessRunner", "state.task_runner"] {
            assert!(
                !source.contains(forbidden),
                "{name} should not directly construct concrete process runner detail {forbidden}"
            );
        }
    }
}

#[test]
fn queue_scheduler_starts_processes_through_port() {
    let scheduler_source = include_str!("../src/application/queue_scheduler_outcomes.rs");
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");
    let request_source = include_str!("../src/application/task_process_start_request.rs");
    let port_source = include_str!("../src/ports/process_runner.rs");
    let adapter_source = include_str!("../src/adapters/task_runner.rs");

    assert!(
        scheduling_ports_source.contains("application::TaskProcessRunner")
            && scheduling_ports_source.contains("TaskProcessRunner"),
        "queue scheduling ports should depend on TaskProcessRunner port"
    );
    assert!(
        !scheduler_source.contains("QueueSchedulingPorts")
            && !scheduler_source.contains("TaskProcessRunner"),
        "queue_scheduler_outcomes should own scheduling outcome/request types only; runtime scheduling is behind QueueSchedulingPorts"
    );
    for forbidden in [
        "use crate::adapters::task_runner::TaskRunner",
        "tauri::AppHandle",
    ] {
        assert!(
            !scheduler_source.contains(forbidden),
            "queue_scheduler should not depend on process adapter detail {forbidden}"
        );
    }

    assert!(port_source.contains("trait TaskProcessRunner"));
    assert!(
        request_source.contains("struct TaskProcessStartRequest"),
        "application layer should own the process start request"
    );
    assert!(
        port_source.contains("application::task_process_start_request::TaskProcessStartRequest"),
        "process runner port should accept an application start request"
    );
    assert!(
        !port_source.contains("pub(crate) struct TaskProcessStartRequest"),
        "process runner port should not own application request models"
    );
    assert!(
        !port_source.contains("use crate::domain::task::Task"),
        "process runner port should not expose the full domain Task entity"
    );
    assert!(adapter_source.contains("impl TaskProcessRunner for TauriTaskProcessRunner"));
}

#[test]
fn process_runner_models_shutdown_status_explicitly() {
    let port_source = include_str!("../src/ports/process_runner.rs");
    let outcome_source = include_str!("../src/application/process_runner_outcomes.rs");

    assert!(
        outcome_source.contains("ProcessRunnerShutdownStatus"),
        "process runner should model shutdown state with an explicit outcome"
    );
    assert!(
        port_source.contains("application::process_runner_outcomes::ProcessRunnerShutdownStatus"),
        "process runner port should use application-owned shutdown status"
    );
    assert!(
        !port_source.contains("enum ProcessRunnerShutdownStatus"),
        "process runner shutdown status should be owned by the application layer, not the port file"
    );
    assert!(
        !port_source.contains("fn is_shutting_down<'a>(&'a self) -> ProcessRunnerFuture<'a, bool>"),
        "process runner shutdown state should not be exposed as a bare bool"
    );
}

#[test]
fn exit_paths_use_process_supervisor_port() {
    let exit_source = include_str!("../src/application/exit_use_cases.rs");
    let exit_orchestrator_source = include_str!("../src/application/exit_orchestrator.rs");
    let port_source = include_str!("../src/ports/process_runner.rs");
    let adapter_source = include_str!("../src/adapters/task_runner.rs");

    assert!(
        exit_source.contains("ExitPorts")
            && exit_orchestrator_source.contains("application::TaskProcessSupervisor"),
        "exit use cases should depend on the process supervisor port"
    );
    assert!(
        exit_orchestrator_source.contains("application::ApplicationControl"),
        "exit use cases should depend on the application control port"
    );
    assert!(
        exit_orchestrator_source.contains("application::QueueRepository"),
        "exit use cases should depend on the queue repository port"
    );
    assert!(
        !exit_source.contains("TaskRunner"),
        "exit use cases should not depend on concrete process adapter"
    );
    assert!(
        !exit_source.contains("QueueManager"),
        "exit use cases should not depend on concrete queue adapter"
    );
    for forbidden in ["tauri::", "AppHandle"] {
        assert!(
            !exit_source.contains(forbidden),
            "exit use cases should not depend on Tauri app control detail {forbidden}"
        );
    }
    assert!(
        !exit_source.contains("task_runner"),
        "ExitUseCases should use task_process_supervisor for exit"
    );
    assert!(
        exit_orchestrator_source.contains("application::ApplicationControl"),
        "ExitUseCases should depend on the application control port"
    );
    assert!(
        exit_orchestrator_source.contains("application::QueueRepository"),
        "ExitUseCases should depend on the queue repository port"
    );
    assert!(
        exit_orchestrator_source.contains("application::SettingsRepository"),
        "ExitUseCases should depend on the settings repository port"
    );
    assert!(
        exit_orchestrator_source.contains("application::ShutdownScheduler"),
        "ExitUseCases should depend on the shutdown scheduler port"
    );
    for forbidden in [
        "pub(crate) settings_repository",
        "pub(crate) queue_repository",
        "pub(crate) shutdown_scheduler",
        "pub(crate) process_supervisor",
        "pub(crate) application_control",
        "pub(crate) diagnostics",
        "pub(crate) events",
    ] {
        assert!(
            !exit_orchestrator_source.contains(forbidden),
            "ExitPorts should encapsulate exit fields instead of exposing {forbidden}"
        );
    }
    for forbidden in [
        "self.ports.settings_repository",
        "self.ports.application_control",
        "ports.process_supervisor",
        "ports.queue_repository",
        "ports.diagnostics",
        "ports.application_control",
    ] {
        assert!(
            !exit_source.contains(forbidden),
            "ExitUseCases should call ExitPorts methods instead of reading {forbidden}"
        );
    }
    assert!(
        !exit_source.contains("use crate::ports::"),
        "ExitUseCases should use the application-owned ExitPorts bundle instead of individual port imports"
    );
    assert!(
        exit_orchestrator_source.contains("AutoShutdownPorts::new")
            && !exit_source.contains("AutoShutdownPorts::new"),
        "ExitPorts should own auto-shutdown bundle construction"
    );
    assert!(
        !exit_orchestrator_source.contains("fn auto_shutdown_ports")
            && !exit_source.contains("auto_shutdown_ports()"),
        "ExitPorts should not expose nested auto-shutdown bundles to exit use cases"
    );
    assert!(
        !exit_source.contains("use crate::composition::dependency_graph::DependencyGraph"),
        "ExitUseCases should not depend on the DependencyGraph container"
    );
    for forbidden in [
        "tauri::",
        "AppHandle",
        "use crate::adapters::window_actions",
    ] {
        assert!(
            !exit_source.contains(forbidden),
            "ExitUseCases should not depend on Tauri window detail {forbidden}"
        );
    }
    assert!(port_source.contains("trait TaskProcessSupervisor"));
    assert!(adapter_source.contains("impl TaskProcessSupervisor for TaskRunner"));
    assert!(
        exit_orchestrator_source.contains("pub(crate) async fn exit_application"),
        "ExitPorts should expose an exit_application intent method for ExitApplication action"
    );
    assert!(
        exit_source.contains("self.ports.exit_application()"),
        "ExitUseCases should call exit_application on ExitPorts for ExitApplication action"
    );
    for forbidden in [
        "self.ports.begin_shutdown()",
        "self.ports.prepare_for_exit()",
        "self.ports.terminate_all_running_processes()",
        "self.ports.warn(",
        "self.ports.exit(",
        "ports.begin_shutdown()",
        "ports.prepare_for_exit()",
        "ports.terminate_all_running_processes()",
        "ports.warn(",
        "ports.exit(",
    ] {
        assert!(
            !exit_source.contains(forbidden),
            "ExitUseCases should not call low-level ExitPorts method {forbidden} directly"
        );
    }
    for forbidden in [
        "Failed to persist queue state before exit",
        "Failed to terminate running processes during exit",
    ] {
        assert!(
            !exit_source.contains(forbidden),
            "ExitUseCases should not contain warning string \"{forbidden}\" - it belongs in ExitPorts"
        );
    }
    for required in [
        "fn mark_exit_queue_state_persistence_failed(",
        "self.mark_exit_queue_state_persistence_failed(&err)",
        "fn mark_exit_running_processes_termination_failed(",
        "self.mark_exit_running_processes_termination_failed(&err)",
    ] {
        assert!(
            exit_orchestrator_source.contains(required),
            "ExitPorts should route exit warning detail through semantic marker {required}"
        );
    }
    for forbidden in ["fn warn(&self", "self.warn("] {
        assert!(
            !exit_orchestrator_source.contains(forbidden),
            "ExitPorts should not retain generic warning helper {forbidden}; use exit-specific markers"
        );
    }
}

#[test]
fn runtime_terminal_history_paths_use_history_repository_port() {
    // queue_scheduler_outcomes owns schedule outcome/request types only.
    // Runtime history lookup is behind QueueSchedulingPorts.
    let queue_scheduler_source = include_str!("../src/application/queue_scheduler_outcomes.rs");
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");
    assert!(
        !queue_scheduler_source.contains("QueueSchedulingPorts")
            && !queue_scheduler_source.contains("HistoryRepository"),
        "queue_scheduler_outcomes should not depend on runtime port bundles; history lookup belongs in QueueSchedulingPorts"
    );
    assert!(
        scheduling_ports_source.contains("application::HistoryRepository")
            || scheduling_ports_source.contains("use crate::application::HistoryRepository"),
        "queue_scheduling_orchestrator should depend on the history repository port"
    );
    for forbidden in [
        "use crate::adapters::history_store::HistoryStore",
        "history_store",
    ] {
        assert!(
            !queue_scheduler_source.contains(forbidden),
            "queue_scheduler_outcomes should not depend on concrete history adapter detail {forbidden}"
        );
        assert!(
            !scheduling_ports_source.contains(forbidden),
            "queue_scheduling_orchestrator should not depend on concrete history adapter detail {forbidden}"
        );
    }

    // terminal_history files depend on TerminalHistoryPorts or history repository port
    for (name, source) in [
        (
            "terminal_history_use_cases.rs",
            include_str!("../src/application/terminal_history_use_cases.rs"),
        ),
        (
            "terminal_history_orchestrator.rs",
            include_str!("../src/application/terminal_history_orchestrator.rs"),
        ),
    ] {
        assert!(
            source.contains("application::HistoryRepository")
                || source.contains("TerminalHistoryPorts"),
            "{name} should depend on the history repository port"
        );
        for forbidden in [
            "use crate::adapters::history_store::HistoryStore",
            "history_store",
        ] {
            assert!(
                !source.contains(forbidden),
                "{name} should not depend on concrete history adapter detail {forbidden}"
            );
        }
    }

    let port_source = include_str!("../src/ports/history_repository.rs");
    let adapter_source = include_str!("../src/adapters/history_store.rs");
    assert!(port_source.contains("trait HistoryRepository"));
    assert!(adapter_source.contains("impl HistoryRepository for HistoryStore"));
}

#[test]
fn terminal_history_uses_queue_repository_port() {
    let source = include_str!("../src/application/terminal_history_use_cases.rs");
    let terminal_history_orchestrator_source =
        include_str!("../src/application/terminal_history_orchestrator.rs");
    let port_source = include_str!("../src/ports/queue_repository.rs");
    let adapter_source = include_str!("../src/adapters/queue_manager.rs");

    assert!(
        source.contains("TerminalHistoryPorts")
            && terminal_history_orchestrator_source.contains("application::QueueRepository"),
        "terminal_history should depend on the queue repository through its terminal history port bundle"
    );
    for forbidden in [
        "pub(crate) queue_repository",
        "pub(crate) history_repository",
    ] {
        assert!(
            !terminal_history_orchestrator_source.contains(forbidden),
            "TerminalHistoryPorts should encapsulate repository fields instead of exposing {forbidden}"
        );
    }
    for forbidden in [
        "ports.queue_repository",
        "ports.history_repository",
        "terminal_history_orchestrator.queue_repository",
        "terminal_history_orchestrator.history_repository",
    ] {
        assert!(
            !source.contains(forbidden),
            "terminal history use cases should call TerminalHistoryPorts methods instead of directly reading {forbidden}"
        );
    }
    assert!(
        !source.contains("queue_manager"),
        "terminal_history should not depend on concrete queue manager storage"
    );
    assert!(
        port_source.contains(
            "pending_history_tasks<'a>(&'a self) -> QueueRepositoryFuture<'a, Vec<TaskSnapshot>>"
        ),
        "queue repository pending-history reads should expose application task snapshots"
    );
    assert!(
        !source.contains("use crate::domain::task::Task"),
        "terminal history use cases should not consume domain task entities from pending-history reads"
    );
    assert!(port_source.contains("trait QueueRepository"));
    // After split: QueueManager implements the narrow traits (no monolithic hand-written `impl QueueRepository for QueueManager`).
    assert!(
        adapter_source.contains("impl QueueStateReader for QueueManager")
            && adapter_source.contains("impl QueueMutation for QueueManager")
            && adapter_source.contains("impl QueueRunLifecycle for QueueManager"),
        "QueueManager should implement the narrow Queue* traits that compose the repository"
    );
    // New boundary checks: use cases should not import staging outcome types
    for forbidden in [
        "QueueTaskCompletionStagingOutcome",
        "QueueTerminalHistoryStagingOutcome",
    ] {
        assert!(
            !source.contains(forbidden),
            "terminal_history_use_cases should not import low-level queue staging outcome types: {forbidden}"
        );
    }
    // Use cases should not call low-level port methods
    for forbidden in [
        "ports.stage_task_completion",
        "ports.stage_terminal_history_task",
        "ports.pending_history_tasks",
        "ports.append_history_task",
        "ports.clear_pending_history_task",
    ] {
        assert!(
            !source.contains(forbidden),
            "terminal_history_use_cases should call high-level port methods instead of {forbidden}"
        );
    }
    // TerminalHistoryPorts should expose high-level methods
    for required in [
        "handle_completed_task_history",
        "handle_terminal_failure_task_history",
        "handle_pending_history_flush",
        "pub(crate) async fn handle_task_failure_transition(",
        "handle_task_failure_transition_error",
    ] {
        assert!(
            terminal_history_orchestrator_source.contains(required),
            "TerminalHistoryPorts should expose high-level method: {required}"
        );
    }
    assert!(
        !terminal_history_orchestrator_source.contains("flush_pending_history_tasks_to_history"),
        "TerminalHistoryPorts should hide history-persistence wording behind semantic intent"
    );
    assert!(
        !terminal_history_orchestrator_source
            .contains("pub(crate) async fn pause_after_failure_persistence_error"),
        "TerminalHistoryPorts should hide queue persistence-error wording behind semantic intent"
    );
    assert!(
        !terminal_history_orchestrator_source.contains("pub(crate) async fn prepare_task_failure"),
        "TerminalHistoryPorts should hide queue repository failure-preparation wording behind semantic intent"
    );
    assert!(
        !terminal_history_orchestrator_source.contains("record_completed_task_to_history"),
        "TerminalHistoryPorts should not retain record_completed_task_to_history; use terminal-history intent naming"
    );
    assert!(
        !source.contains(".record_completed_task_to_history("),
        "terminal_history_use_cases should not call record_completed_task_to_history; use terminal-history intent naming"
    );
    assert!(
        !terminal_history_orchestrator_source.contains("record_terminal_failure_task_to_history"),
        "TerminalHistoryPorts should not retain record_terminal_failure_task_to_history; use terminal-history intent naming"
    );
    assert!(
        !source.contains(".record_terminal_failure_task_to_history("),
        "terminal_history_use_cases should not call record_terminal_failure_task_to_history; use terminal-history intent naming"
    );
    assert!(
        source.contains("pub(crate) async fn handle_completed_task_history("),
        "terminal_history_use_cases should expose handle_completed_task_history"
    );
    assert!(
        !source.contains("pub(crate) async fn record_completed_task("),
        "terminal_history_use_cases should not retain record_completed_task; use terminal-history intent naming"
    );
    assert!(
        source.contains("pub(crate) async fn handle_terminal_failure_task_history("),
        "terminal_history_use_cases should expose handle_terminal_failure_task_history"
    );
    assert!(
        !source.contains("pub(crate) async fn record_terminal_failure_task("),
        "terminal_history_use_cases should not retain record_terminal_failure_task; use terminal-history intent naming"
    );
    // Low-level helpers should be private (not pub(crate))
    for forbidden in [
        "pub(crate) async fn stage_task_completion",
        "pub(crate) async fn stage_terminal_history_task",
        "pub(crate) async fn pending_history_tasks",
        "pub(crate) fn append_history_task",
        "pub(crate) async fn clear_pending_history_task",
    ] {
        assert!(
            !terminal_history_orchestrator_source.contains(forbidden),
            "TerminalHistoryPorts low-level helper should be private, not: {forbidden}"
        );
    }
}

#[test]
fn queue_shutdown_status_helper_moved_to_scheduling_ports() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");
    let app_mod_source = include_str!("../src/application/mod.rs");

    // Verify queue_run_ports.rs has been removed
    assert!(
        !app_mod_source.contains("pub(crate) mod queue_run_ports;"),
        "queue_run_ports module should be removed from application/mod.rs"
    );

    // QueueSchedulingPorts calls queue_repository.shutdown_status directly
    assert!(
        scheduling_ports_source.contains("self.queue_repository.shutdown_status().await"),
        "QueueSchedulingPorts should contain direct self.queue_repository.shutdown_status().await"
    );
    // QueueSchedulingPorts uses bare bool for shutdown status (downgraded)
    assert!(
        scheduling_ports_source.contains("self.queue_repository.shutdown_status().await"),
        "QueueSchedulingPorts should call self.queue_repository.shutdown_status().await directly"
    );
    // queue_run_ports.rs has been removed - verify through app_mod
    let app_mod_source = include_str!("../src/application/mod.rs");
    assert!(
        !app_mod_source.contains("pub(crate) mod queue_run_ports;"),
        "queue_run_ports module should be removed"
    );
}

#[test]
fn process_runner_shutdown_status_helper_moved_to_scheduling_ports() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");

    // QueueSchedulingPorts calls process_runner.shutdown_status directly
    assert!(
        scheduling_ports_source.contains("self.process_runner.shutdown_status().await"),
        "QueueSchedulingPorts should contain direct self.process_runner.shutdown_status().await"
    );
    // QueueSchedulingPorts maps ProcessRunnerShutdownStatus variants explicitly
    assert!(
        scheduling_ports_source.contains("ProcessRunnerShutdownStatus::ShuttingDown"),
        "QueueSchedulingPorts should map ProcessRunnerShutdownStatus::ShuttingDown"
    );
    assert!(
        scheduling_ports_source.contains("ProcessRunnerShutdownStatus::Running"),
        "QueueSchedulingPorts should map ProcessRunnerShutdownStatus::Running"
    );
    // queue_run_ports.rs has been removed
    let app_mod_source = include_str!("../src/application/mod.rs");
    assert!(
        !app_mod_source.contains("pub(crate) mod queue_run_ports;"),
        "queue_run_ports module should be removed"
    );
}

#[test]
fn queue_scheduling_owns_warning_diagnostics() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");

    assert!(
        !scheduling_ports_source.contains("fn warn(&self"),
        "QueueSchedulingPorts should not contain a generic fn warn helper"
    );
    assert!(
        !scheduling_ports_source.contains("self.warn("),
        "QueueSchedulingPorts should not call self.warn(...)"
    );
    assert!(
        scheduling_ports_source.contains("self.diagnostics.warn("),
        "QueueSchedulingPorts should call diagnostics.warn directly for queue-scheduling warnings"
    );
}

#[test]
fn queue_scheduling_owns_queue_state_changed_event() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");

    assert!(
        scheduling_ports_source.contains("self.events.queue_state_changed()"),
        "QueueSchedulingPorts should publish queue-state-changed directly through FrontendEventPublisher"
    );
    assert!(
        !scheduling_ports_source.contains("fn queue_state_changed(&self)"),
        "QueueSchedulingPorts should not have a generic queue_state_changed helper"
    );
    assert!(
        !scheduling_ports_source.contains("self.queue_state_changed()"),
        "QueueSchedulingPorts should not call self.queue_state_changed()"
    );
}

#[test]
fn queue_scheduling_owns_task_error_event() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");

    assert!(
        scheduling_ports_source.contains("self.events.task_error(task_id, message)"),
        "QueueSchedulingPorts should publish task-error directly through FrontendEventPublisher"
    );
    assert!(
        !scheduling_ports_source.contains("fn task_error(&self"),
        "QueueSchedulingPorts should not contain a generic task_error helper"
    );
    assert!(
        !scheduling_ports_source.contains("self.task_error("),
        "QueueSchedulingPorts should not call self.task_error()"
    );
}

#[test]
fn task_lifecycle_orchestrator_no_low_level_child_failure_outcome() {
    let lifecycle_ports_source = include_str!("../src/application/task_lifecycle_orchestrator.rs");
    // TaskLifecyclePorts should not import ExitedChildFailureOutcome
    assert!(
        !lifecycle_ports_source.contains("ExitedChildFailureOutcome"),
        "TaskLifecyclePorts must not contain ExitedChildFailureOutcome"
    );
}

#[test]
fn task_lifecycle_orchestrator_uses_queue_scheduling_semantic_intent() {
    let lifecycle_ports_source = include_str!("../src/application/task_lifecycle_orchestrator.rs");
    // TaskLifecyclePorts should call the failed child-exit continuation intent.
    assert!(
        lifecycle_ports_source.contains(".handle_failed_child_exit(task_id, error_message)"),
        "TaskLifecyclePorts should call the new task-lifecycle semantic intent method"
    );
}

#[test]
fn queue_scheduling_owns_failed_child_exit_matching() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");
    let method_start = scheduling_ports_source
        .find("async fn handle_failed_child_exit_internal")
        .expect("handle_failed_child_exit_internal not found");
    let method_end = scheduling_ports_source[method_start..]
        .find("/// High-level intent method for completed-child-exit history recording")
        .map(|offset| method_start + offset)
        .expect("failed-child-exit intent method region end not found");
    let method_region = &scheduling_ports_source[method_start..method_end];
    let method_region_compact = method_region.split_whitespace().collect::<String>();

    assert!(
        method_region.contains("handle_failed_child_exit_internal"),
        "QueueSchedulingPorts should keep handle_failed_child_exit_internal as an internal method"
    );
    assert!(
        !scheduling_ports_source.contains("pub(crate) async fn handle_failed_child_exit_internal"),
        "QueueSchedulingPorts should not expose handle_failed_child_exit_internal as pub(crate)"
    );
    assert!(
        scheduling_ports_source.contains("pub(crate) async fn handle_failed_child_exit("),
        "QueueSchedulingPorts should expose handle_failed_child_exit"
    );
    assert!(
        scheduling_ports_source.contains("async fn fail_child_exit"),
        "QueueSchedulingPorts should keep failed-child-exit sequencing behind an internal helper"
    );
    assert!(
        !scheduling_ports_source.contains("pub(crate) async fn fail_child_exit"),
        "QueueSchedulingPorts should not expose fail_child_exit as pub(crate)"
    );
    let failed_intent_start = scheduling_ports_source
        .find("pub(crate) async fn handle_failed_child_exit")
        .expect("failed child-exit continuation intent function not found");
    let failed_intent_end = scheduling_ports_source[failed_intent_start..]
        .find("async fn fail_child_exit")
        .map(|offset| failed_intent_start + offset)
        .expect("failed child-exit helper function header not found");
    let failed_intent_fn = &scheduling_ports_source[failed_intent_start..failed_intent_end];
    let failed_intent_fn_compact = failed_intent_fn.split_whitespace().collect::<String>();
    assert!(
        failed_intent_fn_compact
            .contains("self.fail_child_exit(task_id,error_message).await;"),
        "handle_failed_child_exit should delegate failed-child-exit sequencing to the internal helper"
    );
    for forbidden in [
        "clear_child_exit_terminal_active_line",
        "continue_child_exit_unless_shutting_down",
        "handle_failed_child_exit_internal",
        "drive_child_exit_queue_and_handle_shutdown_countdown",
    ] {
        assert!(
            !failed_intent_fn.contains(forbidden),
            "handle_failed_child_exit should not contain lower-level failed-child-exit detail {forbidden}"
        );
    }
    let failed_driver_start = scheduling_ports_source
        .find("async fn fail_child_exit")
        .expect("failed child-exit sequencing helper not found");
    let failed_driver_end = scheduling_ports_source[failed_driver_start..]
        .find("async fn acknowledge_shutdown_child_exit_if_needed")
        .map(|offset| failed_driver_start + offset)
        .expect("shutdown acknowledgement helper function header not found");
    let failed_driver_fn = &scheduling_ports_source[failed_driver_start..failed_driver_end];
    let failed_driver_fn_compact = failed_driver_fn.split_whitespace().collect::<String>();
    assert!(
        failed_driver_fn_compact.contains("self.clear_child_exit_terminal_active_line(task_id);"),
        "fail_child_exit should clear terminal active line before continuation"
    );
    assert!(
        failed_driver_fn_compact.contains("self.continue_child_exit_unless_shutting_down"),
        "fail_child_exit should use the shutdown-aware child-exit continuation"
    );
    assert!(
        failed_driver_fn_compact
            .contains("self.handle_failed_child_exit_internal(task_id,error_message).await;"),
        "fail_child_exit should call handle_failed_child_exit_internal"
    );
    assert!(
        failed_driver_fn_compact.contains(
            "self.drive_child_exit_queue_and_handle_shutdown_countdown(\"failure\").await;"
        ),
        "fail_child_exit should drive run-completion continuation internally"
    );
    assert!(
        method_region_compact.contains("Some(ExitedChildFailureOutcome::RetryScheduled)|Some(ExitedChildFailureOutcome::Ignored)=>{}"),
        "handle_failed_child_exit_internal should ignore RetryScheduled and Ignored outcomes"
    );
    let scheduling_ports_source_compact = scheduling_ports_source
        .split_whitespace()
        .collect::<String>();
    assert!(
        scheduling_ports_source.contains("fn mark_terminal_child_exit_failure"),
        "QueueSchedulingPorts should define mark_terminal_child_exit_failure internally"
    );
    assert!(
        !scheduling_ports_source.contains("pub(crate) fn mark_terminal_child_exit_failure"),
        "QueueSchedulingPorts should not expose terminal failure marking as pub(crate)"
    );
    assert!(
        scheduling_ports_source_compact.contains(
            "fnmark_terminal_child_exit_failure(&self){self.shutdown_scheduler.mark_run_failure();}"
        ),
        "QueueSchedulingPorts should own shutdown failure marking for terminal child exits"
    );
    let terminal_branch = "Some(ExitedChildFailureOutcome::Terminal)=>{self.mark_terminal_child_exit_failure();self.handle_failed_child_exit_history(task_id).await;}";
    assert!(
        method_region_compact.contains(terminal_branch),
        "handle_failed_child_exit_internal should mark terminal failure before recording failed history"
    );
    assert!(
        !scheduling_ports_source.contains("on_terminal"),
        "QueueSchedulingPorts should not accept a terminal callback; it owns terminal failure marking"
    );
    assert!(
        method_region_compact.contains("None=>{}"),
        "handle_failed_child_exit_internal should not record failed history after prepare-failure warning"
    );
}

#[test]
fn queue_scheduling_owns_history_task_added_event() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");
    let scheduling_ports_source_compact = scheduling_ports_source
        .split_whitespace()
        .collect::<String>();

    assert!(
        scheduling_ports_source_compact
            .contains("self.events.history_task_added(HistoryStatus::Completed,task)")
            && scheduling_ports_source_compact
                .contains("self.events.history_task_added(HistoryStatus::Failed,task)"),
        "QueueSchedulingPorts should publish history-task-added directly through FrontendEventPublisher with explicit status values"
    );
    assert!(
        !scheduling_ports_source
            .contains(".publish_history_task_added_for_queue_scheduling(status, task)"),
        "QueueSchedulingPorts should not delegate history-task-added through QueueRunPorts"
    );
    assert!(
        !scheduling_ports_source.contains("fn history_task_added(&self")
            && !scheduling_ports_source.contains("async fn history_task_added(&self"),
        "QueueSchedulingPorts should not contain a history_task_added helper method"
    );
    assert!(
        !scheduling_ports_source.contains("self.history_task_added("),
        "QueueSchedulingPorts should not call self.history_task_added()"
    );
}

#[test]
fn queue_scheduling_owns_shutdown_countdown_events() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");
    let queue_start_orchestrator_source =
        include_str!("../src/application/queue_start_orchestrator.rs");

    assert!(
        queue_start_orchestrator_source.contains("fn mark_queue_start_shutdown_countdown_cancelled"),
        "QueueStartPorts should own queue-start shutdown-countdown-cancelled through a semantic marker"
    );
    assert!(
        !queue_start_orchestrator_source
            .contains("pub(crate) fn mark_queue_start_shutdown_countdown_cancelled"),
        "QueueStartPorts should not expose mark_queue_start_shutdown_countdown_cancelled"
    );
    assert!(
        !scheduling_ports_source.contains("self.events.shutdown_countdown_cancelled()"),
        "QueueSchedulingPorts should not publish queue-start shutdown-countdown-cancelled directly"
    );
    assert!(
        !scheduling_ports_source.contains("fn shutdown_countdown_cancelled(&self"),
        "QueueSchedulingPorts should not have private shutdown_countdown_cancelled helper"
    );
    assert!(
        !scheduling_ports_source.contains("async fn shutdown_countdown_cancelled(&self"),
        "QueueSchedulingPorts should not have private async shutdown_countdown_cancelled helper"
    );
    assert!(
        !scheduling_ports_source.contains("self.shutdown_countdown_cancelled("),
        "QueueSchedulingPorts should not call self.shutdown_countdown_cancelled()"
    );
    assert!(
        !scheduling_ports_source
            .contains(".publish_shutdown_countdown_cancelled_for_queue_start()"),
        "QueueSchedulingPorts should not delegate shutdown-countdown-cancelled through QueueRunPorts"
    );
    assert!(
        scheduling_ports_source.contains("self.events.shutdown_countdown_started(seconds)"),
        "QueueSchedulingPorts should publish shutdown-countdown-started directly through FrontendEventPublisher"
    );
    assert!(
        !scheduling_ports_source
            .contains(".publish_shutdown_countdown_started_for_run_completion(seconds)"),
        "QueueSchedulingPorts should not delegate shutdown-countdown-started through QueueRunPorts"
    );
    assert!(
        !scheduling_ports_source.contains("fn shutdown_countdown_started(&self"),
        "QueueSchedulingPorts should not have private shutdown_countdown_started helper"
    );
    assert!(
        !scheduling_ports_source.contains("async fn shutdown_countdown_started(&self"),
        "QueueSchedulingPorts should not have private async shutdown_countdown_started helper"
    );
    assert!(
        !scheduling_ports_source.contains("self.shutdown_countdown_started("),
        "QueueSchedulingPorts should not call self.shutdown_countdown_started()"
    );
}

#[test]
fn queue_process_start_helper_moved_to_scheduling_ports() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");

    // QueueSchedulingPorts should call self.process_runner.start_task directly, not through run_ports
    assert!(
        scheduling_ports_source.contains("self.process_runner.start_task(request).await"),
        "QueueSchedulingPorts should call self.process_runner.start_task(request).await directly"
    );
    // QueueSchedulingPorts should NOT call run_ports().start_task()
    assert!(
        !scheduling_ports_source.contains("self.run_ports().start_task(request).await"),
        "QueueSchedulingPorts should not delegate start_task through run_ports"
    );
}

#[test]
fn queue_repository_models_run_finish_outcome_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");

    assert!(
        port_source.contains("AppResult<bool>"),
        "queue repository should model run-finish decisions as bool (downgraded outcome)"
    );
}

#[test]
fn queue_repository_models_pending_history_clear_outcome_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");

    assert!(
        port_source.contains("AppResult<bool>"),
        "queue repository should model pending-history clear decisions as bool (downgraded outcome)"
    );
}

#[test]
fn queue_run_finish_helper_moved_to_scheduling_ports() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");

    // QueueSchedulingPorts should call self.queue_repository.finish_run_if_idle().await directly
    assert!(
        scheduling_ports_source.contains("self.queue_repository.finish_run_if_idle().await"),
        "QueueSchedulingPorts should call self.queue_repository.finish_run_if_idle().await directly"
    );

    // QueueSchedulingPorts should not call self.run_ports().finish_run_if_idle().await
    assert!(
        !scheduling_ports_source.contains("self.run_ports().finish_run_if_idle().await"),
        "QueueSchedulingPorts should not call self.run_ports().finish_run_if_idle().await"
    );
}

#[test]
fn queue_repository_models_add_task_outcome_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");
    let outcome_source = include_str!("../src/application/queue_repository_outcomes.rs");

    assert!(
        port_source.contains("AppResult<bool>"),
        "queue repository should model add-task scheduling decisions as bool (downgraded outcome)"
    );
    assert!(
        !port_source.contains("AddTaskOutcome"),
        "queue repository should no longer mention the downgraded AddTaskOutcome enum"
    );
    // Domain still owns the internal AddTaskOutcome for state machine; port no longer surfaces the app-level one.
    let _ = outcome_source;
}

#[test]
fn queue_repository_retry_returns_task_snapshot() {
    let port_source = include_str!("../src/ports/queue_repository.rs");

    assert!(
        port_source
            .contains("retry_task<'a>(&'a self, id: &'a str) -> QueueRepositoryFuture<'a, AppResult<TaskSnapshot>>"),
        "queue retry reads should expose application task snapshots"
    );
    assert!(
        !port_source.contains(
            "retry_task<'a>(&'a self, id: &'a str) -> QueueRepositoryFuture<'a, AppResult<Task>>"
        ),
        "queue retry should not expose domain task entities through the repository port"
    );
}

#[test]
fn queue_scheduler_models_schedule_request_explicitly() {
    let scheduler_source = include_str!("../src/application/queue_scheduler_outcomes.rs");

    assert!(
        scheduler_source.contains("enum ScheduleNextRequest"),
        "queue scheduler should model schedule requests explicitly"
    );
    assert!(
        !scheduler_source.contains("should_schedule: bool"),
        "queue scheduler should not receive schedule requests as a bare bool"
    );
}

#[test]
fn queue_scheduler_models_schedule_outcome_explicitly() {
    let scheduler_source = include_str!("../src/application/queue_scheduler_outcomes.rs");

    assert!(
        scheduler_source.contains("enum ScheduleNextOutcome"),
        "queue scheduler should model schedule outcomes explicitly"
    );
    assert!(
        !scheduler_source.contains("fn queue_changed(&self) -> bool"),
        "schedule outcome should not be collapsed back into a bare bool helper"
    );
    assert!(
        !scheduler_source.contains("queue_state_changed: bool")
            && !scheduler_source.contains("let mut queue_state_changed = false"),
        "queue scheduler should not track schedule outcomes with a bare bool"
    );
}

#[test]
fn queue_repository_models_schedule_next_outcome_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");
    let outcome_source = include_str!("../src/application/queue_repository_outcomes.rs");

    assert!(
        port_source.contains("AppResult<Option<TaskSnapshot>>"),
        "queue repository should model schedule-next decisions as Option<TaskSnapshot> (downgraded outcome)"
    );
    assert!(
        !port_source.contains("QueueScheduleNextOutcome"),
        "queue repository should no longer mention the downgraded QueueScheduleNextOutcome enum"
    );
    let _ = outcome_source;
}

#[test]
fn queue_repository_models_task_failure_preparation_outcome_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");
    let _outcome_source = include_str!("../src/application/queue_repository_outcomes.rs");

    assert!(
        port_source.contains("PrepareTaskFailureOutcome"),
        "queue repository should model task-failure preparation decisions with an explicit application outcome"
    );
    // PrepareTaskFailureOutcome and TaskFailureTransition still live in application outcomes (not downgraded in T-05).
    assert!(
        !port_source.contains("use crate::domain::queue::PrepareTaskFailureOutcome")
            && !port_source.contains("use crate::domain::queue::{AddTaskOutcome, PrepareTaskFailureOutcome, QueueRunStatus"),
        "queue repository should not import task-failure preparation outcome from domain"
    );
    assert!(
        !port_source.contains("AppResult<Option<TaskFailureTransition>>"),
        "queue repository should not encode task-failure preparation decisions as Option<TaskFailureTransition>"
    );
}

#[test]
fn queue_repository_models_completion_staging_outcome_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");

    assert!(
        port_source.contains("Option<TaskSnapshot>"),
        "queue repository should model completion staging decisions as Option<TaskSnapshot> (downgraded outcome)"
    );
    assert!(
        !port_source.contains("QueueTaskCompletionStagingOutcome"),
        "queue repository should no longer mention the downgraded QueueTaskCompletionStagingOutcome enum"
    );
}

#[test]
fn queue_repository_models_terminal_history_staging_outcome_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");

    assert!(
        port_source.contains("Option<TaskSnapshot>"),
        "queue repository should model terminal-history staging decisions as Option<TaskSnapshot> (downgraded outcome)"
    );
    assert!(
        !port_source.contains("QueueTerminalHistoryStagingOutcome"),
        "queue repository should no longer mention the downgraded QueueTerminalHistoryStagingOutcome enum"
    );
}

#[test]
fn queue_repository_models_live_progress_update_outcome_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");
    let outcome_source = include_str!("../src/application/queue_repository_outcomes.rs");

    assert!(
        port_source.contains("bool"),
        "queue repository should model live progress update decisions as bare bool (downgraded outcome)"
    );
    assert!(
        !port_source.contains("QueueLiveTaskProgressUpdateOutcome"),
        "queue repository should no longer mention the downgraded QueueLiveTaskProgressUpdateOutcome enum"
    );
    let _ = outcome_source;
}

#[test]
fn queue_repository_models_shutdown_status_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");
    let outcome_source = include_str!("../src/application/queue_repository_outcomes.rs");
    let domain_queue_source = include_str!("../src/domain/queue.rs");

    assert!(
        port_source.contains("bool"),
        "queue repository should model shutdown state as bare bool (downgraded status)"
    );
    assert!(
        !port_source.contains("QueueShutdownStatus"),
        "queue repository should no longer mention the downgraded QueueShutdownStatus enum"
    );
    // Domain must not define the old app-level status.
    assert!(
        !domain_queue_source.contains("enum QueueShutdownStatus"),
        "queue shutdown status is runtime/application state, not a domain entity"
    );
    let _ = outcome_source;
}

#[test]
fn queue_repository_models_live_work_status_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");
    let outcome_source = include_str!("../src/application/queue_repository_outcomes.rs");

    assert!(
        port_source.contains("bool"),
        "queue repository should model live-work state as bare bool (downgraded status)"
    );
    assert!(
        !port_source.contains("QueueLiveWorkStatus"),
        "queue repository should no longer mention the downgraded QueueLiveWorkStatus enum"
    );
    let _ = outcome_source;
}

#[test]
fn queue_repository_models_run_status_explicitly() {
    let port_source = include_str!("../src/ports/queue_repository.rs");
    let outcome_source = include_str!("../src/application/queue_repository_outcomes.rs");
    let domain_queue_source = include_str!("../src/domain/queue.rs");

    assert!(
        domain_queue_source.contains("QueueRunStatus"),
        "queue running/paused state should be modeled as a domain status"
    );
    assert!(
        domain_queue_source.contains("run_status: QueueRunStatus"),
        "QueueAggregate should store running/paused as an explicit domain status"
    );
    assert!(
        !domain_queue_source.contains("pub is_running: bool"),
        "QueueAggregate should not store running/paused state as a bare bool"
    );
    assert!(
        !domain_queue_source.contains("pub run_status: QueueRunStatus"),
        "QueueAggregate should not expose run status as a public field"
    );
    assert!(
        port_source.contains("QueueRunStatus"),
        "queue repository should accept an explicit application run status"
    );
    assert!(
        outcome_source.contains("enum QueueRunStatus")
            && outcome_source.contains("Running")
            && outcome_source.contains("Paused"),
        "queue repository run command should be application-owned"
    );
    assert!(
        !port_source.contains(
            "use crate::domain::queue::{AddTaskOutcome, PrepareTaskFailureOutcome, QueueRunStatus"
        ) && !port_source.contains("use crate::domain::queue::QueueRunStatus"),
        "queue repository should not import run status from domain"
    );
    assert!(
        !port_source.contains("fn set_running<'a>(&'a self, running: bool)"),
        "queue repository should not expose running/paused transitions as a bare bool"
    );
}

#[test]
fn queue_scheduling_owns_terminal_active_line_event() {
    // QueueSchedulingPorts publishes terminal_active_line directly through self.events
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");
    // With empty string for child-exit clear intent
    assert!(
        scheduling_ports_source
            .contains("self.events.terminal_active_line(task_id, \"\")"),
        "QueueSchedulingPorts should publish terminal_active_line with empty string for child-exit clear intent"
    );
    // QueueSchedulingPorts should NOT have terminal_active_line helper anymore
    assert!(
        !scheduling_ports_source.contains("fn terminal_active_line(&self")
            && !scheduling_ports_source.contains("async fn terminal_active_line(&self"),
        "QueueSchedulingPorts should not have terminal_active_line private helper"
    );
    // QueueSchedulingPorts should NOT call self.terminal_active_line(
    assert!(
        !scheduling_ports_source.contains("self.terminal_active_line("),
        "QueueSchedulingPorts should not call self.terminal_active_line()"
    );
    // application/mod.rs must not declare queue_run_ports module
    let app_mod_source = include_str!("../src/application/mod.rs");
    assert!(
        !app_mod_source.contains("pub(crate) mod queue_run_ports;"),
        "application/mod.rs must not declare pub(crate) mod queue_run_ports"
    );
    // TaskLifecyclePorts should use completed/failed child-exit intent methods
    // instead of calling the active-line clear helper directly.
    let lifecycle_ports_source = include_str!("../src/application/task_lifecycle_orchestrator.rs");
    assert!(
        !lifecycle_ports_source
            .contains(".clear_child_exit_terminal_active_line_for_task_lifecycle"),
        "TaskLifecyclePorts should not call clear_child_exit_terminal_active_line_for_task_lifecycle directly"
    );
}

#[test]
fn dependency_graph_exposes_queue_as_repository_port() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::queue_repository::QueueRepository"),
        "DependencyGraph should expose queue through the repository port"
    );
    assert!(
        source.contains("Arc<dyn QueueRepository>"),
        "DependencyGraph should hold a queue repository trait object"
    );
    assert!(
        !source.contains("use crate::adapters::queue_manager::QueueManager"),
        "DependencyGraph should not depend on concrete queue storage"
    );
}

#[test]
fn dependency_graph_exposes_history_as_repository_port() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::history_repository::HistoryRepository"),
        "DependencyGraph should expose history through the repository port"
    );
    assert!(
        source.contains("Arc<dyn HistoryRepository>"),
        "DependencyGraph should hold a history repository trait object"
    );
    assert!(
        !source.contains("use crate::adapters::history_store::HistoryStore"),
        "DependencyGraph should not depend on concrete history storage"
    );
}

#[test]
fn dependency_graph_exposes_terminal_output_as_repository_port() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::terminal_output_repository::TerminalOutputRepository"),
        "DependencyGraph should expose terminal output through the repository port"
    );
    assert!(
        source.contains("Arc<dyn TerminalOutputRepository>"),
        "DependencyGraph should hold a terminal output repository trait object"
    );
    assert!(
        !source.contains("use crate::adapters::cli_output_store::CliOutputStore"),
        "DependencyGraph should not depend on concrete CLI output storage"
    );
}

#[test]
fn dependency_graph_exposes_settings_as_repository_port() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::settings_repository::SettingsRepository"),
        "DependencyGraph should expose settings through the repository port"
    );
    assert!(
        source.contains("Arc<dyn SettingsRepository>"),
        "DependencyGraph should hold a settings repository trait object"
    );
    assert!(
        !source.contains("use crate::adapters::settings_store::SettingsStore"),
        "DependencyGraph should not depend on concrete settings storage"
    );
}

#[test]
fn dependency_graph_exposes_directory_opener_as_port() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::directory_opener::DirectoryOpener"),
        "DependencyGraph should expose directory opening through a port"
    );
    assert!(
        source.contains("Arc<dyn DirectoryOpener>"),
        "DependencyGraph should hold a directory opener trait object"
    );
    assert!(
        !source.contains("use crate::adapters::system_directory_opener::SystemDirectoryOpener"),
        "DependencyGraph should not depend on concrete directory opener adapter"
    );
}

#[test]
fn dependency_graph_exposes_download_directory_resolver_as_port() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::download_directory_resolver::DownloadDirectoryResolver"),
        "DependencyGraph should expose download directory resolution through a port"
    );
    assert!(
        source.contains("Arc<dyn DownloadDirectoryResolver>"),
        "DependencyGraph should hold a download directory resolver trait object"
    );
    assert!(
        !source
            .contains("use crate::adapters::system_download_directory_resolver::SystemDownloadDirectoryResolver"),
        "DependencyGraph should not depend on concrete download directory adapter"
    );
}

#[test]
fn dependency_graph_exposes_application_control_as_port() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::application_control::ApplicationControl"),
        "DependencyGraph should expose application control through a port"
    );
    assert!(
        source.contains("Arc<dyn ApplicationControl>"),
        "DependencyGraph should hold an application control trait object"
    );
    assert!(
        !source.contains("use crate::adapters::tauri_application_control::TauriApplicationControl"),
        "DependencyGraph should not depend on concrete Tauri application control adapter"
    );
}

#[test]
fn dependency_graph_exposes_shutdown_as_scheduler_port() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::shutdown_scheduler::ShutdownScheduler"),
        "DependencyGraph should expose shutdown through the scheduler port"
    );
    assert!(
        source.contains("Arc<dyn ShutdownScheduler>"),
        "DependencyGraph should hold a shutdown scheduler trait object"
    );
    assert!(
        !source.contains("use crate::adapters::shutdown::ShutdownManager"),
        "DependencyGraph should not depend on concrete shutdown adapter"
    );
}

#[test]
fn dependency_graph_exposes_process_shutdown_as_supervisor_port() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::process_runner"),
        "DependencyGraph should expose process capabilities through process runner ports"
    );
    assert!(source.contains("TaskProcessRunnerFactory"));
    assert!(source.contains("TaskProcessSupervisor"));
    assert!(
        source.contains("Arc<dyn TaskProcessSupervisor>"),
        "DependencyGraph should hold a process supervisor trait object"
    );
    assert!(
        source.contains("Arc<dyn TaskProcessRunnerFactory>"),
        "DependencyGraph should hold a process runner factory trait object"
    );
    assert!(
        !source.contains("use crate::adapters::task_runner::TaskRunner"),
        "DependencyGraph should not depend on concrete task runner adapter"
    );
}

#[test]
fn dependency_graph_exposes_task_identity_and_clock_as_ports() {
    let source = include_str!("../src/composition/dependency_graph.rs");

    assert!(
        source.contains("ports::task_id_generator::TaskIdGenerator"),
        "DependencyGraph should expose task id generation through a port"
    );
    assert!(
        source.contains("Arc<dyn TaskIdGenerator>"),
        "DependencyGraph should hold a task id generator trait object"
    );
    assert!(
        source.contains("ports::clock::Clock"),
        "DependencyGraph should expose time through a clock port"
    );
    assert!(
        source.contains("Arc<dyn Clock>"),
        "DependencyGraph should hold a clock trait object"
    );
    assert!(
        source.contains("fn task_creation_orchestrator")
            && source.contains("TaskCreationPorts::new"),
        "DependencyGraph should expose task identity and time as an application task creation port bundle"
    );
    assert!(
        !source.contains("UuidTaskIdGenerator") && !source.contains("SystemClock"),
        "DependencyGraph should not depend on concrete task identity or clock adapters"
    );
}

#[test]
fn shutdown_scheduler_models_reset_outcome_explicitly() {
    let port_source = include_str!("../src/ports/shutdown_scheduler.rs");
    let outcome_source = include_str!("../src/application/shutdown_scheduler_outcomes.rs");

    assert!(
        outcome_source.contains("ShutdownResetOutcome"),
        "shutdown scheduler should model reset decisions with an explicit outcome"
    );
    assert!(
        port_source.contains("application::shutdown_scheduler_outcomes::"),
        "shutdown scheduler port should use application-owned shutdown outcome types"
    );
    assert!(
        !port_source.contains("enum ShutdownResetOutcome"),
        "shutdown reset outcome should be owned by the application layer, not the port file"
    );
    assert!(
        !port_source.contains("fn reset_for_new_run(&self) -> AppResult<bool>"),
        "shutdown reset should not encode countdown-cancelled semantics as a bare bool"
    );
}

#[test]
fn shutdown_scheduler_models_countdown_start_decision_explicitly() {
    let port_source = include_str!("../src/ports/shutdown_scheduler.rs");
    let outcome_source = include_str!("../src/application/shutdown_scheduler_outcomes.rs");

    assert!(
        outcome_source.contains("ShutdownCountdownStartDecision"),
        "shutdown scheduler should model countdown-start decisions with an explicit outcome"
    );
    assert!(
        port_source.contains("application::shutdown_scheduler_outcomes::"),
        "shutdown scheduler port should use application-owned countdown decision types"
    );
    assert!(
        !port_source.contains("enum ShutdownCountdownStartDecision"),
        "shutdown countdown decision should be owned by the application layer, not the port file"
    );
    assert!(
        !port_source.contains("fn should_start_countdown(&self) -> bool"),
        "shutdown countdown start should not encode start/block semantics as a bare bool"
    );
}

#[test]
fn terminal_output_repository_models_active_line_explicitly() {
    let port_source = include_str!("../src/ports/terminal_output_repository.rs");
    let outcome_source = include_str!("../src/application/terminal_output_outcomes.rs");

    assert!(
        outcome_source.contains("TerminalActiveLine"),
        "terminal output repository should model active-line presence with an explicit outcome"
    );
    assert!(
        port_source.contains("application::terminal_output_outcomes::TerminalActiveLine"),
        "terminal output repository port should use application-owned active-line outcome"
    );
    assert!(
        !port_source.contains("enum TerminalActiveLine"),
        "terminal active-line outcome should be owned by the application layer, not the port file"
    );
    assert!(
        !port_source.contains("fn get_active_line(&self, task_id: &str) -> Option<String>"),
        "terminal active line should not encode present/missing semantics as Option<String>"
    );
}

#[test]
fn terminal_history_core_has_no_tauri_worker_dependencies() {
    let source = include_str!("../src/application/terminal_history_use_cases.rs");

    for forbidden in [
        "tauri::",
        "AppHandle",
        "TauriFrontendEventPublisher",
        "spawn_pending_history_flush",
        "use crate::adapters::tauri_frontend_event_publisher",
    ] {
        assert!(
            !source.contains(forbidden),
            "terminal_history core should not depend on Tauri worker detail {forbidden}"
        );
    }

    assert!(
        source.contains("Recorded(TaskSnapshot)"),
        "terminal history record outcomes should expose snapshots instead of domain tasks"
    );
    assert!(
        source.contains("AppResult<Vec<FlushedHistoryTask>>"),
        "pending history flush should expose flushed task snapshots with status"
    );

    let worker_source = include_str!("../src/composition/pending_history_worker.rs");
    assert!(worker_source.contains("spawn_pending_history_flush"));
    assert!(worker_source.contains("TauriFrontendEventPublisher"));
}

#[test]
fn pending_history_worker_delegates_flush_to_pending_history_facade() {
    let source = include_str!("../src/composition/pending_history_worker.rs");
    let bootstrap_source = include_str!("../src/composition/app_bootstrap.rs");
    let dependency_graph_source = include_str!("../src/composition/dependency_graph.rs");
    let pending_history_facade_source =
        include_str!("../src/composition/pending_history_facade.rs");

    assert!(source.contains("composition::dependency_graph::DependencyGraph"));
    assert!(source.contains("composition::pending_history_facade::PendingHistoryFacade"));
    assert!(source.contains("PendingHistoryFacade::new"));
    assert!(bootstrap_source.contains("spawn_pending_history_flush(state.clone()"));
    assert!(pending_history_facade_source
        .contains("terminal_history_use_cases::flush_pending_history_tasks"));
    assert!(
        pending_history_facade_source.contains("terminal_history_orchestrator")
            && dependency_graph_source.contains("TerminalHistoryPorts::new"),
        "PendingHistoryFacade should obtain terminal-history port wiring from DependencyGraph"
    );
    assert!(
        !pending_history_facade_source.contains("use crate::domain::task::Task"),
        "PendingHistoryFacade should not expose domain task entities to adapters"
    );
    assert!(
        !dependency_graph_source.contains("flush_pending_history_tasks"),
        "DependencyGraph should not expose pending history use-case orchestration"
    );
    for forbidden in [
        "ports::queue_repository::QueueRepository",
        "ports::history_repository::HistoryRepository",
        "application::terminal_history_use_cases::flush_pending_history_tasks",
        "terminal_history_use_cases::flush_pending_history_tasks",
        "use crate::adapters::queue_manager::QueueManager",
        "queue_repository.as_ref()",
        "history_repository.as_ref()",
        "state.queue_manager",
    ] {
        assert!(
            !source.contains(forbidden),
            "pending history worker should delegate {forbidden} through PendingHistoryFacade"
        );
    }
}

#[test]
fn queue_manager_delegates_retry_rules_to_domain_policy() {
    let queue_manager_source = include_str!("../src/adapters/queue_manager.rs");
    let domain_queue_source = include_str!("../src/domain/queue.rs");

    assert!(
        domain_queue_source.contains("RetryPolicy"),
        "QueueAggregate should apply retry decisions from domain::retry_policy"
    );
    assert!(
        !queue_manager_source.contains("retry_count <"),
        "retry thresholds should live in domain::retry_policy, not QueueManager"
    );
    assert!(
        !queue_manager_source.contains("RetryPolicy"),
        "QueueManager should delegate retry failure transitions to QueueAggregate"
    );
}

#[test]
fn queue_manager_does_not_own_queue_state_transition_rules() {
    let queue_manager_source = include_str!("../src/adapters/queue_manager.rs");
    let production_source = queue_manager_source
        .split("#[cfg(test)]")
        .next()
        .expect("queue manager production source");

    for forbidden in [
        "TaskStatus::",
        ".status =",
        ".progress =",
        ".speed =",
        ".threads =",
        ".error_message =",
        ".current_task_id =",
        ".is_running =",
        ".pending_history_tasks",
    ] {
        assert!(
            !production_source.contains(forbidden),
            "QueueManager should delegate {forbidden} transitions to QueueAggregate"
        );
    }
}

#[test]
fn queue_manager_does_not_create_tasks() {
    let production_source = include_str!("../src/adapters/queue_manager.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("queue manager production source");

    for forbidden in ["Task::new_queued", "Uuid::new_v4", "Utc::now"] {
        assert!(
            !production_source.contains(forbidden),
            "QueueManager should persist application-created tasks, not create tasks with {forbidden}"
        );
    }
}

#[test]
fn queue_manager_delegates_repository_outcome_mapping() {
    let production_source = include_str!("../src/adapters/queue_manager.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("queue manager production source");
    let mapper_source = include_str!("../src/adapters/queue_repository_mappers.rs");

    assert!(
        production_source.contains("queue_repository_mappers"),
        "QueueManager should delegate repository outcome mapping to an adapter mapper"
    );
    for mapper in [
        "fn domain_run_status",
        "fn application_add_task_outcome",
        "fn application_prepare_task_failure_outcome",
        "fn application_run_finish_outcome",
        "fn application_pending_history_clear_outcome",
        "fn application_schedule_next_outcome",
        "fn application_task_completion_staging_outcome",
        "fn application_terminal_history_staging_outcome",
        "fn application_remove_task_result",
        "fn application_retry_task_result",
    ] {
        assert!(
            !production_source.contains(mapper),
            "QueueManager should not own mapper implementation {mapper}"
        );
        assert!(
            mapper_source.contains(mapper),
            "queue_repository_mappers should own mapper implementation {mapper}"
        );
    }
}

#[test]
fn queue_manager_delegates_state_storage() {
    let production_source = include_str!("../src/adapters/queue_manager.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("queue manager production source");
    let store_source = include_str!("../src/adapters/queue_state_store.rs");

    assert!(
        production_source.contains("QueueStateStore"),
        "QueueManager should delegate queue state locking and persistence to QueueStateStore"
    );
    for forbidden in [
        "Persistence::load",
        "Persistence::save",
        "Arc<Mutex<QueueAggregate>>",
        "QueueStateUpdate",
        "fn commit_state",
        "fn persist",
    ] {
        assert!(
            !production_source.contains(forbidden),
            "QueueManager should not own state-store detail {forbidden}"
        );
        assert!(
            store_source.contains(forbidden),
            "QueueStateStore should own state-store detail {forbidden}"
        );
    }
}

#[test]
fn queue_manager_delegates_shutdown_gate() {
    let production_source = include_str!("../src/adapters/queue_manager.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("queue manager production source");
    let gate_source = include_str!("../src/adapters/queue_shutdown_gate.rs");

    assert!(
        production_source.contains("QueueShutdownGate"),
        "QueueManager should delegate runtime shutdown flag storage to QueueShutdownGate"
    );
    for forbidden in [
        "Arc<Mutex<bool>>",
        "async fn mark_shutting_down",
        "async fn clear_shutdown_flag",
    ] {
        assert!(
            !production_source.contains(forbidden),
            "QueueManager should not own shutdown gate detail {forbidden}"
        );
        assert!(
            gate_source.contains(forbidden),
            "QueueShutdownGate should own shutdown gate detail {forbidden}"
        );
    }
}

#[test]
fn persistence_adapter_delegates_restart_normalization_to_domain() {
    let production_source = include_str!("../src/adapters/persistence.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("persistence production source");

    assert!(
        production_source.contains(".normalize_after_restart()"),
        "persistence adapter should ask QueueAggregate to normalize restored runtime state"
    );
    for forbidden in [
        "TaskStatus::Downloading",
        ".status =",
        ".current_task_id =",
        ".is_running =",
    ] {
        assert!(
            !production_source.contains(forbidden),
            "persistence adapter should not own restart state rule {forbidden}"
        );
    }
}

#[test]
fn task_entity_and_status_are_owned_by_domain_layer() {
    let domain_task_source = include_str!("../src/domain/task.rs");

    assert!(
        domain_task_source.contains("pub struct Task"),
        "domain::task should own Task"
    );
    assert!(
        domain_task_source.contains("pub enum TaskStatus"),
        "domain::task should own TaskStatus"
    );
}

#[test]
fn task_entity_and_status_do_not_depend_on_serialization_details() {
    let domain_task_source = include_str!("../src/domain/task.rs");

    for forbidden in ["serde", "Serialize", "Deserialize", "#[serde"] {
        assert!(
            !domain_task_source.contains(forbidden),
            "domain::task Task/TaskStatus should not depend on serialization detail {forbidden}"
        );
    }
}

#[test]
fn task_entity_does_not_contain_runtime_fields() {
    let domain_task_source = include_str!("../src/domain/task.rs");

    for forbidden in ["progress", "speed", "threads", "output_path"] {
        assert!(
            !domain_task_source.contains(forbidden),
            "domain::task Task should not contain runtime field {forbidden}"
        );
    }
}

#[test]
fn task_runtime_state_contains_all_runtime_fields() {
    let runtime_state_source = include_str!("../src/application/task_runtime_state.rs");

    assert!(
        runtime_state_source.contains("pub(crate) progress: f32"),
        "TaskRuntimeState should contain progress field"
    );
    assert!(
        runtime_state_source.contains("pub(crate) speed: String"),
        "TaskRuntimeState should contain speed field"
    );
    assert!(
        runtime_state_source.contains("pub(crate) threads: String"),
        "TaskRuntimeState should contain threads field"
    );
    assert!(
        runtime_state_source.contains("pub(crate) output_path: Option<String>"),
        "TaskRuntimeState should contain output_path field"
    );
}

#[test]
fn domain_layer_does_not_depend_on_task_runtime_state() {
    let domain_queue_source = include_str!("../src/domain/queue.rs");
    let domain_task_source = include_str!("../src/domain/task.rs");

    for forbidden in ["TaskRuntimeState", "task_runtime_state"] {
        assert!(
            !domain_queue_source.contains(forbidden),
            "domain::queue should not reference application-layer type {forbidden}"
        );
        assert!(
            !domain_task_source.contains(forbidden),
            "domain::task should not reference application-layer type {forbidden}"
        );
    }
}

#[test]
fn runtime_states_are_managed_in_adapter_layer() {
    let queue_state_store_source = include_str!("../src/adapters/queue_state_store.rs");

    assert!(
        queue_state_store_source.contains("runtime_states"),
        "QueueStateStore should store runtime_states"
    );
    assert!(
        queue_state_store_source.contains("TaskRuntimeState"),
        "QueueStateStore should use TaskRuntimeState type"
    );
}

#[test]
fn adapters_layer_does_not_depend_on_composition_layer() {
    let adapters_dir = std::path::Path::new("src/adapters");
    for entry in std::fs::read_dir(adapters_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains("crate::composition"),
            "Adapter file {:?} must not import from composition layer",
            path.file_name().unwrap()
        );
    }
}

#[test]
fn history_status_is_owned_by_domain_layer() {
    let domain_history_source = include_str!("../src/domain/history.rs");

    assert!(
        domain_history_source.contains("pub enum HistoryStatus"),
        "domain::history should own HistoryStatus"
    );
}

#[test]
fn history_status_does_not_depend_on_serialization_details() {
    let domain_history_source = include_str!("../src/domain/history.rs");

    for forbidden in ["serde", "Serialize", "Deserialize", "#[serde"] {
        assert!(
            !domain_history_source.contains(forbidden),
            "domain::history HistoryStatus should not depend on serialization detail {forbidden}"
        );
    }
    assert!(
        !domain_history_source.contains("as_str"),
        "domain::history should not own adapter string representations"
    );

    let adapter_codec_source = include_str!("../src/adapters/history_status_codec.rs");
    assert!(
        adapter_codec_source.contains("parse_history_status")
            && adapter_codec_source.contains("history_status_slug"),
        "adapter layer should own history status string parsing and formatting"
    );
}

#[test]
fn cli_output_parsers_are_adapter_details() {
    let domain_modules = include_str!("../src/domain/mod.rs");
    let adapter_modules = include_str!("../src/adapters/mod.rs");

    for parser in ["progress_parser", "terminal_parser"] {
        assert!(
            !domain_modules.contains(parser),
            "external CLI output parser {parser} should not be a domain module"
        );
        assert!(
            adapter_modules.contains(parser),
            "external CLI output parser {parser} should live with process adapters"
        );
    }
}

#[test]
fn queue_aggregate_is_owned_by_domain_layer() {
    let domain_queue_source = include_str!("../src/domain/queue.rs");
    let aggregate_declaration = domain_queue_source
        .split("struct QueueAggregate")
        .nth(1)
        .and_then(|source| {
            source
                .split("pub(crate) enum StageTerminalHistoryResult")
                .next()
        })
        .expect("queue aggregate declaration source");
    let aggregate_impl = domain_queue_source
        .split("impl QueueAggregate")
        .nth(1)
        .and_then(|source| source.split("impl Default").next())
        .expect("queue aggregate implementation source");

    assert!(
        domain_queue_source.contains("pub(crate) struct QueueAggregate"),
        "domain::queue should own QueueAggregate as an internal aggregate"
    );
    assert!(
        !domain_queue_source.contains("type QueueState = QueueAggregate"),
        "QueueState compatibility alias should be retired from the domain boundary"
    );
    assert!(
        domain_queue_source.contains("push_pending_history_task"),
        "QueueAggregate should own pending history invariants"
    );
    assert!(
        domain_queue_source.contains("pub(crate) struct QueuePendingHistory"),
        "QueueAggregate should model pending history as an explicit domain collection"
    );
    assert!(
        !domain_queue_source.contains("pub pending_history_tasks: Vec<Task>"),
        "QueueAggregate should not expose pending history as a raw Vec field"
    );
    assert!(
        !domain_queue_source.contains("pub tasks: Vec<Task>"),
        "QueueAggregate should not expose queued tasks as a public Vec field"
    );
    assert!(
        domain_queue_source.contains("pub(crate) struct QueueTasks"),
        "QueueAggregate should model queued tasks as an explicit domain collection"
    );
    assert!(
        !domain_queue_source.contains("impl Deref for QueueTasks"),
        "QueueTasks should not expose queued tasks through implicit slice deref"
    );
    assert!(
        !aggregate_declaration.contains("tasks: Vec<Task>"),
        "QueueAggregate should not store queued tasks directly as a raw Vec field"
    );
    for delegated_task_rule in [
        "fn schedule_next_download",
        "fn reorder_waiting_tasks",
        "fn reset_downloading_tasks_for_exit",
    ] {
        assert!(
            domain_queue_source.contains(delegated_task_rule),
            "QueueTasks should own queued-task transition helper {delegated_task_rule}"
        );
    }
    assert!(
        !aggregate_impl.contains(".iter().position("),
        "QueueAggregate should delegate queued-task lookups to QueueTasks"
    );
}

#[test]
fn queue_aggregate_current_task_transitions_are_centralized() {
    let production_source = include_str!("../src/domain/queue.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .expect("domain queue production source");

    assert!(
        production_source.contains("fn assign_current_task")
            && production_source.contains("fn clear_current_task_if_matches"),
        "QueueAggregate should centralize current-task assignment and clearing"
    );
    assert!(
        !production_source.contains("current_task_id.is_none()"),
        "QueueAggregate should not scatter current-task absence checks as raw Option checks"
    );
    assert!(
        !production_source.contains("pub current_task_id: Option<String>"),
        "QueueAggregate should model current task as an explicit domain state, not a raw Option field"
    );
    assert_eq!(
        production_source.matches("self.current_task =").count(),
        2,
        "QueueAggregate should write current_task only inside assignment/clear helpers"
    );
}

#[test]
fn queue_aggregate_does_not_depend_on_serialization_details() {
    let domain_queue_source = include_str!("../src/domain/queue.rs");

    for forbidden in ["serde", "Serialize", "Deserialize", "#[serde"] {
        assert!(
            !domain_queue_source.contains(forbidden),
            "domain::queue QueueAggregate should not depend on serialization detail {forbidden}"
        );
    }
}

#[test]
fn add_task_payload_is_owned_by_application_layer() {
    let application_source = include_str!("../src/application/queue_requests.rs");

    assert!(
        application_source.contains("pub struct AddTaskPayload"),
        "application layer should own AddTaskPayload as a use-case request"
    );
}

#[test]
fn add_task_payload_does_not_depend_on_serialization_details() {
    let application_source = include_str!("../src/application/queue_requests.rs");

    for forbidden in ["serde", "Serialize", "Deserialize", "#[serde"] {
        assert!(
            !application_source.contains(forbidden),
            "AddTaskPayload should be a use-case request, not a transport DTO {forbidden}"
        );
    }
}

#[test]
fn app_settings_are_owned_by_application_layer() {
    let application_source = include_str!("../src/application/settings.rs");

    assert!(
        application_source.contains("pub struct AppSettings"),
        "application layer should own AppSettings"
    );
    assert!(
        application_source.contains("pub enum CloseButtonBehavior"),
        "application layer should own close-button policy"
    );
}

#[test]
fn app_settings_do_not_depend_on_serialization_details() {
    let application_source = include_str!("../src/application/settings.rs");

    for forbidden in ["serde", "Serialize", "Deserialize", "#[serde"] {
        assert!(
            !application_source.contains(forbidden),
            "AppSettings should be an application model, not a transport or storage DTO {forbidden}"
        );
    }
}

#[test]
fn app_error_is_owned_by_application_layer() {
    let application_source = include_str!("../src/application/app_error.rs");
    let application_mod_source = include_str!("../src/application/mod.rs");
    let ports_mod_source = include_str!("../src/ports/mod.rs");

    assert!(
        application_source.contains("pub enum AppError")
            && application_source.contains("pub type AppResult"),
        "application layer should own shared use-case error types"
    );
    assert!(
        application_mod_source.contains("mod app_error"),
        "application module should expose app_error internally"
    );
    assert!(
        !ports_mod_source.contains("mod app_error"),
        "ports module should not own shared error types"
    );
}

#[test]
fn terminal_output_use_cases_depend_on_repository_port() {
    let source = include_str!("../src/application/terminal_output_use_cases.rs");
    let ports_source = include_str!("../src/application/terminal_output_orchestrator.rs");
    let page_source = include_str!("../src/application/terminal_output_page.rs");
    let port_source = include_str!("../src/ports/terminal_output_repository.rs");
    let adapter_source = include_str!("../src/adapters/cli_output_store.rs");

    assert!(source.contains("TerminalOutputPorts"));
    assert!(ports_source.contains("application::TerminalOutputRepository"));
    assert!(ports_source.contains("TerminalOutputRepository"));
    assert!(
        !ports_source.contains("pub(crate) terminal_output_repository"),
        "TerminalOutputPorts should encapsulate repository fields"
    );
    assert!(
        !source.contains("self.ports.terminal_output_repository"),
        "TerminalOutputUseCases should call TerminalOutputPorts methods instead of reading repository fields"
    );
    assert!(source.contains("CliTerminalState"));
    assert!(
        page_source.contains("struct TerminalOutputPage"),
        "application layer should own terminal output page data"
    );
    assert!(
        port_source.contains("application::terminal_output_page::TerminalOutputPage"),
        "terminal output repository port should return application page data, not frontend DTOs"
    );
    assert!(
        !port_source.contains("pub(crate) struct TerminalOutputPage"),
        "terminal output repository port should not own application page models"
    );
    assert!(
        !port_source.contains("application::query_models::CliOutputPage"),
        "terminal output repository port should not return frontend CliOutputPage DTOs"
    );
    assert!(
        !adapter_source.contains("application::query_models"),
        "terminal output storage adapter should not construct frontend query models"
    );
    for forbidden in [
        "use crate::adapters::cli_output_store::CliOutputStore",
        "use crate::adapters::tauri_frontend_event_publisher",
        "tauri::",
    ] {
        assert!(
            !source.contains(forbidden),
            "terminal output use cases should not depend on adapter detail {forbidden}"
        );
    }
}

#[test]
fn history_use_cases_depend_on_repository_port() {
    let source = include_str!("../src/application/history_use_cases.rs");
    let history_orchestrator_source = include_str!("../src/application/history_orchestrator.rs");
    let page_source = include_str!("../src/application/history_task_page.rs");
    let port_source = include_str!("../src/ports/history_repository.rs");
    let adapter_source = include_str!("../src/adapters/history_store.rs");

    assert!(source.contains("HistoryPorts"));
    assert!(history_orchestrator_source.contains("application::HistoryRepository"));
    assert!(
        !history_orchestrator_source.contains("pub(crate) history_repository"),
        "HistoryPorts should encapsulate repository fields"
    );
    assert!(
        !source.contains("self.ports.history_repository"),
        "HistoryUseCases should call HistoryPorts methods instead of reading repository fields"
    );
    assert!(source.contains("HistoryPage"));
    assert!(
        page_source.contains("struct HistoryTaskPage"),
        "application layer should own history task page data"
    );
    assert!(
        page_source.contains("Vec<TaskSnapshot>"),
        "history task pages should carry application task snapshots"
    );
    assert!(
        !page_source.contains("Vec<Task>"),
        "history task pages should not expose domain task entities on read paths"
    );
    assert!(
        port_source.contains("application::history_task_page::HistoryTaskPage"),
        "history repository port should return application page data, not frontend DTOs"
    );
    assert!(
        port_source.contains("fn append(&self, task: &TaskSnapshot)"),
        "history repository append should accept application task snapshots"
    );
    assert!(
        !port_source.contains("use crate::domain::task::Task"),
        "history repository port should not expose domain task entities"
    );
    assert!(
        adapter_source.contains("stored_tasks_from_snapshots"),
        "history storage adapter should persist history snapshots"
    );
    assert!(
        !port_source.contains("pub(crate) struct HistoryTaskPage"),
        "history repository port should not own application page models"
    );
    assert!(
        !port_source.contains("application::query_models::HistoryPage"),
        "history repository port should not return frontend HistoryPage DTOs"
    );
    assert!(
        !adapter_source.contains("application::query_models"),
        "history storage adapter should not construct frontend query models"
    );
    for forbidden in [
        "use crate::adapters::history_store::HistoryStore",
        "use crate::adapters::tauri_frontend_event_publisher",
        "tauri::",
    ] {
        assert!(
            !source.contains(forbidden),
            "history use cases should not depend on adapter detail {forbidden}"
        );
    }
}

#[test]
fn history_repository_models_remove_outcome_explicitly() {
    let port_source = include_str!("../src/ports/history_repository.rs");
    let outcome_source = include_str!("../src/application/history_repository_outcomes.rs");
    let domain_source = include_str!("../src/domain/history.rs");

    assert!(
        port_source.contains("HistoryRemoveOutcome"),
        "history repository should model remove decisions with an explicit outcome"
    );
    assert!(
        outcome_source.contains("enum HistoryRemoveOutcome"),
        "application layer should own history remove repository outcomes"
    );
    assert!(
        port_source.contains("application::history_repository_outcomes::"),
        "history repository port should use application-owned repository outcomes"
    );
    assert!(
        !domain_source.contains("HistoryRemoveOutcome"),
        "domain history should not own repository remove outcomes"
    );
    assert!(
        !port_source.contains(
            "fn remove_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<bool>"
        ),
        "history remove_task should not encode removed/missing semantics as a bare bool"
    );
}

#[test]
fn history_repository_models_find_outcome_explicitly() {
    let port_source = include_str!("../src/ports/history_repository.rs");
    let outcome_source = include_str!("../src/application/history_repository_outcomes.rs");
    let domain_source = include_str!("../src/domain/history.rs");
    let task_creation_orchestrator_source =
        include_str!("../src/application/task_creation_orchestrator.rs");

    assert!(
        port_source.contains("HistoryFindOutcome"),
        "history repository should model find decisions with an explicit outcome"
    );
    assert!(
        outcome_source.contains("enum HistoryFindOutcome"),
        "application layer should own history find repository outcomes"
    );
    assert!(
        port_source.contains("application::history_repository_outcomes::"),
        "history repository port should use application-owned repository outcomes"
    );
    assert!(
        !domain_source.contains("HistoryFindOutcome"),
        "domain history should not own repository find outcomes"
    );
    assert!(
        outcome_source.contains("Found(TaskSnapshot)"),
        "history find outcomes should expose application task snapshots"
    );
    assert!(
        !outcome_source.contains("use crate::domain::task::Task"),
        "history find outcomes should not expose domain task entities"
    );
    assert!(
        task_creation_orchestrator_source
            .contains("create_queued_task_from_history_retry")
            && task_creation_orchestrator_source.contains("history_task: &TaskSnapshot"),
        "history retry task creation should consume application snapshots through TaskCreationPorts"
    );
    assert!(
        !port_source.contains(
            "fn find_task(&self, status: HistoryStatus, task_id: &str) -> AppResult<Option<Task>>"
        ),
        "history find_task should not encode found/missing semantics as Option<Task>"
    );
}

#[test]
fn queue_run_status_helpers_moved_to_scheduling_ports() {
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");

    // QueueSchedulingPorts directly contains QueueRunStatus::Paused set_run_status calls
    assert!(
        scheduling_ports_source.contains("set_run_status(QueueRunStatus::Paused)"),
        "QueueSchedulingPorts should directly call set_run_status(QueueRunStatus::Paused)"
    );

    // QueueSchedulingPorts directly contains QueueRunStatus::Running set_run_status calls
    assert!(
        scheduling_ports_source.contains("set_run_status(QueueRunStatus::Running)"),
        "QueueSchedulingPorts should directly call set_run_status(QueueRunStatus::Running)"
    );
}

#[test]
fn query_models_are_owned_by_application_layer() {
    let application_source = include_str!("../src/application/query_models.rs");

    assert!(
        application_source.contains("pub enum TaskStatusView"),
        "application layer should own TaskStatusView"
    );

    for type_name in [
        "TaskView",
        "QueueStateView",
        "HistoryPage",
        "CliOutputPage",
        "CliTerminalState",
    ] {
        assert!(
            application_source.contains(&format!("pub struct {type_name}")),
            "application layer should own {type_name}"
        );
    }

    for forbidden in [
        "use crate::domain::task",
        "From<Task>",
        "From<&Task>",
        "TaskStatus::",
    ] {
        assert!(
            !application_source.contains(forbidden),
            "application query models should project from snapshots instead of domain task detail {forbidden}"
        );
    }
}

#[test]
fn application_query_models_do_not_depend_on_serialization_details() {
    let application_source = include_str!("../src/application/query_models.rs");

    for forbidden in ["serde", "Serialize", "Deserialize", "#[serde"] {
        assert!(
            !application_source.contains(forbidden),
            "application query models should not depend on frontend serialization detail {forbidden}"
        );
    }
}

#[test]
fn tauri_commands_project_application_query_models_to_frontend_dtos() {
    let source = include_str!("../src/composition/tauri_commands.rs");

    assert!(
        source.contains("QueueStateDto"),
        "get_queue_state should return a frontend DTO projected from an application read model"
    );
    assert!(
        source.contains("TaskDto"),
        "add/retry task commands should return frontend DTOs projected from application task read models"
    );
    assert!(
        !source.contains("use crate::domain::queue::QueueState"),
        "Tauri commands should not expose the domain queue aggregate as a frontend DTO"
    );
    assert!(
        !source.contains("use crate::domain::task::Task"),
        "Tauri commands should not expose the domain task entity as a frontend DTO"
    );
}

#[test]
fn task_creation_orchestrator_owns_task_creation_boundary() {
    let ports_source = include_str!("../src/application/task_creation_orchestrator.rs");
    let app_mod_source = include_str!("../src/application/mod.rs");
    let scheduling_ports_source =
        include_str!("../src/application/queue_scheduling_orchestrator.rs");

    for forbidden in ["pub(crate) task_id_generator", "pub(crate) clock"] {
        assert!(
            !ports_source.contains(forbidden),
            "TaskCreationPorts should encapsulate task creation fields instead of exposing {forbidden}"
        );
    }
    for forbidden in ["pub(crate) fn next_task_id", "pub(crate) fn now"] {
        assert!(
            !ports_source.contains(forbidden)
                && !app_mod_source.contains(forbidden)
                && !scheduling_ports_source.contains(forbidden),
            "task creation should not expose or depend on {forbidden}"
        );
    }
    assert!(
        ports_source.contains("pub(crate) fn create_queued_task_from_payload")
            && ports_source.contains("pub(crate) fn create_queued_task_from_history_retry")
            && ports_source.contains("Task::new_queued(")
            && ports_source.contains("self.next_task_id()")
            && ports_source.contains("self.now()"),
        "TaskCreationPorts should own queued task construction behind semantic methods"
    );
}

#[test]
fn models_facade_has_been_removed() {
    let models_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("models.rs");
    assert!(
        !models_path.exists(),
        "models.rs should not reappear as an ownership bypass"
    );
}
