use crate::application::app_error::{AppError, AppResult};
use crate::application::download_directory::DownloadDirectory;
use crate::application::history_repository_outcomes::HistoryFindOutcome;
use crate::application::process_runner_outcomes::ProcessRunnerShutdownStatus;
use crate::application::queue_repository_outcomes::{
    PrepareTaskFailureOutcome, QueueRunStatus, TaskFailureTransition,
};
use crate::application::queue_requests::AddTaskPayload;
use crate::application::queue_scheduler_outcomes::{
    ExitedChildFailureOutcome, ScheduleNextOutcome, ScheduleNextRequest, StartFailureOutcome,
};
use crate::application::settings::AppSettings;
use crate::application::shutdown_scheduler_outcomes::{
    ShutdownCountdownStartDecision, ShutdownResetOutcome,
};
use crate::application::task_creation_orchestrator::TaskCreationPorts;
use crate::application::task_process_start_request::TaskProcessStartRequest;
use crate::application::task_snapshot::TaskSnapshot;
use crate::application::terminal_history_orchestrator::TerminalHistoryPorts;
use crate::application::terminal_history_use_cases::{
    handle_completed_task_history, handle_terminal_failure_task_history,
    TerminalHistoryRecordOutcome,
};
use crate::application::Diagnostics;
use crate::application::ArtifactInventory;
use crate::application::Clock;
use crate::application::DownloadDirectoryResolver;
use crate::application::FrontendEventPublisher;
use crate::application::HistoryRepository;
use crate::application::QueueRepository;
use crate::application::SettingsRepository;
use crate::application::ShutdownScheduler;
use crate::application::TaskProcessRunner;
use crate::application::TerminalOutputRepository;
use crate::domain::history::HistoryStatus;
use crate::domain::task::Task;

#[derive(Clone, Copy)]
pub(crate) struct QueueSchedulingPorts<'a> {
    queue_repository: &'a dyn QueueRepository,
    settings_repository: &'a dyn SettingsRepository,
    download_directory_resolver: &'a dyn DownloadDirectoryResolver,
    history_repository: &'a dyn HistoryRepository,
    terminal_output_repository: &'a dyn TerminalOutputRepository,
    shutdown_scheduler: &'a dyn ShutdownScheduler,
    process_runner: &'a dyn TaskProcessRunner,
    artifact_inventory: &'a dyn ArtifactInventory,
    clock: &'a dyn Clock,
    diagnostics: &'a dyn Diagnostics,
    events: &'a dyn FrontendEventPublisher,
}

/// Identifies which queue mutation triggered a `schedule_next + queue_state_changed` tail.
/// Carries scenario identity for future diagnostics/metrics hooks.
/// Per ADR-0008: private to this file, only used for the homogeneous QueueAdd-Retry tail pattern.
enum QueueMutationScenario {
    QueueAdd,
    RetryFromHistory,
    RetryExisting,
}

impl QueueMutationScenario {
    /// Emit queue-state-changed event for this scenario.
    /// Currently identical across scenarios; the discriminant is preserved as
    /// a future metrics/telemetry hook point (consumed via match, not dead weight).
    fn emit_queue_changed(self, events: &dyn FrontendEventPublisher) {
        match self {
            QueueMutationScenario::QueueAdd
            | QueueMutationScenario::RetryFromHistory
            | QueueMutationScenario::RetryExisting => events.queue_state_changed(),
        }
    }
}

impl<'a> QueueSchedulingPorts<'a> {
    pub(crate) fn new(
        queue_repository: &'a dyn QueueRepository,
        settings_repository: &'a dyn SettingsRepository,
        download_directory_resolver: &'a dyn DownloadDirectoryResolver,
        history_repository: &'a dyn HistoryRepository,
        terminal_output_repository: &'a dyn TerminalOutputRepository,
        shutdown_scheduler: &'a dyn ShutdownScheduler,
        process_runner: &'a dyn TaskProcessRunner,
        artifact_inventory: &'a dyn ArtifactInventory,
        clock: &'a dyn Clock,
        diagnostics: &'a dyn Diagnostics,
        events: &'a dyn FrontendEventPublisher,
    ) -> Self {
        Self {
            queue_repository,
            settings_repository,
            download_directory_resolver,
            history_repository,
            terminal_output_repository,
            shutdown_scheduler,
            process_runner,
            artifact_inventory,
            clock,
            diagnostics,
            events,
        }
    }

    fn terminal_history_orchestrator(&self) -> TerminalHistoryPorts<'_> {
        TerminalHistoryPorts::new(self.queue_repository, self.history_repository)
    }

    /// Internal helper that handles exited-child failure transition sequencing.
    /// Encapsulates: task failure transition outcome matching and transition-error handling.
    /// External task-lifecycle callers should use handle_failed_child_exit instead.
    async fn handle_exited_child_failure_transition(
        &self,
        task_id: &str,
        error_message: &str,
    ) -> AppResult<ExitedChildFailureOutcome> {
        match self
            .handle_task_failure_transition(task_id, error_message)
            .await
        {
            Err(err) => {
                self.handle_task_failure_transition_error(task_id, error_message)
                    .await;
                Err(err)
            }
            Ok(PrepareTaskFailureOutcome::Transition(TaskFailureTransition::RetryScheduled)) => {
                Ok(ExitedChildFailureOutcome::RetryScheduled)
            }
            Ok(PrepareTaskFailureOutcome::Transition(TaskFailureTransition::Terminal)) => {
                Ok(ExitedChildFailureOutcome::Terminal)
            }
            Ok(PrepareTaskFailureOutcome::Ignored) => Ok(ExitedChildFailureOutcome::Ignored),
        }
    }

    /// High-level intent method for task-lifecycle failed-child-exit preparation.
    /// Calls handle_exited_child_failure_transition and records preparation failure on Err.
    /// Returns Some(outcome) on success, None on failure (after warning).
    async fn prepare_failed_child_exit_transition(
        &self,
        task_id: &str,
        error_message: &str,
    ) -> Option<ExitedChildFailureOutcome> {
        match self
            .handle_exited_child_failure_transition(task_id, error_message)
            .await
        {
            Ok(outcome) => Some(outcome),
            Err(err) => {
                self.mark_child_exit_failure_preparation_failed(&err);
                None
            }
        }
    }

    fn mark_child_exit_failure_preparation_failed(&self, error: &dyn std::fmt::Display) {
        self.diagnostics
            .warn(&format!("Failed to prepare task failure: {}", error));
    }

    /// High-level intent method that handles start-failure transition sequencing.
    /// Encapsulates: handle_exited_child_failure_transition, matching ExitedChildFailureOutcome,
    /// and terminal-failure history handling with TerminalHistoryRecordOutcome matching.
    async fn handle_start_failure_transition(
        &self,
        task_id: &str,
        error_message: &str,
    ) -> AppResult<StartFailureOutcome> {
        match self
            .handle_exited_child_failure_transition(task_id, error_message)
            .await?
        {
            ExitedChildFailureOutcome::RetryScheduled => Ok(StartFailureOutcome::RetryScheduled),
            ExitedChildFailureOutcome::Ignored => Ok(StartFailureOutcome::Ignored),
            ExitedChildFailureOutcome::Terminal => self
                .handle_terminal_failure_history(task_id)
                .await
                .map(|outcome| match outcome {
                    TerminalHistoryRecordOutcome::Recorded(task) => {
                        StartFailureOutcome::Terminal(task)
                    }
                    TerminalHistoryRecordOutcome::Ignored => StartFailureOutcome::Ignored,
                }),
        }
    }

    async fn handle_task_failure_transition(
        &self,
        task_id: &str,
        error_message: &str,
    ) -> AppResult<PrepareTaskFailureOutcome> {
        self.terminal_history_orchestrator()
            .handle_task_failure_transition(task_id, error_message)
            .await
    }

    async fn handle_task_failure_transition_error(&self, task_id: &str, error_message: &str) {
        self.terminal_history_orchestrator()
            .handle_task_failure_transition_error(task_id, error_message)
            .await;
    }

    async fn handle_terminal_failure_history(
        &self,
        task_id: &str,
    ) -> AppResult<TerminalHistoryRecordOutcome> {
        handle_terminal_failure_task_history(&self.terminal_history_orchestrator(), task_id).await
    }

    /// High-level intent method for failed-child-exit history recording and notification.
    /// Encapsulates: terminal-failure history handling, TerminalHistoryRecordOutcome matching,
    /// failed-history recorded event marking, failed-history record failure marking.
    async fn handle_failed_child_exit_history(&self, task_id: &str) {
        match self.handle_terminal_failure_history(task_id).await {
            Ok(TerminalHistoryRecordOutcome::Recorded(task)) => {
                self.mark_failed_child_exit_history_recorded(&task);
            }
            Ok(TerminalHistoryRecordOutcome::Ignored) => {}
            Err(err) => {
                let message = format!("任务已失败，但写入失败历史时出错：{}", err);
                self.mark_failed_child_exit_history_record_failed(task_id, &message);
            }
        }
    }

    fn mark_failed_child_exit_history_recorded(&self, task: &TaskSnapshot) {
        self.events.history_task_added(HistoryStatus::Failed, task);
    }

    fn mark_failed_child_exit_history_record_failed(&self, task_id: &str, message: &str) {
        self.diagnostics.warn(message);
        self.events.task_error(task_id, message);
    }

    fn mark_terminal_child_exit_failure(&self) {
        self.shutdown_scheduler.mark_run_failure();
    }

    /// High-level intent method for task-lifecycle failed-child-exit handling.
    /// Handles the transition and marks run failure only for Terminal outcome
    /// after prepare failure transition succeeds but before failed-history recording.
    async fn handle_failed_child_exit_internal(&self, task_id: &str, error_message: &str) {
        match self
            .prepare_failed_child_exit_transition(task_id, error_message)
            .await
        {
            Some(ExitedChildFailureOutcome::RetryScheduled)
            | Some(ExitedChildFailureOutcome::Ignored) => {}
            Some(ExitedChildFailureOutcome::Terminal) => {
                self.mark_terminal_child_exit_failure();
                self.handle_failed_child_exit_history(task_id).await;
            }
            None => {}
        }
    }

    /// High-level intent method for completed-child-exit history recording and notification.
    /// Encapsulates: completed-task history handling, TerminalHistoryRecordOutcome matching,
    /// completed-history recorded event marking, completed-history record failure marking.
    async fn handle_completed_child_exit_history(
        &self,
        task_id: &str,
        output_path: Option<&str>,
        artifact_diagnostic: Option<&crate::application::artifact_resolution::ArtifactDiagnostic>,
    ) {
        // Downstream history chain expects a `&str` (legacy contract, where
        // empty string means "no artifact located"). None → "" preserves
        // that contract; ADR-0005 stage 4 keeps downstream unchanged.
        let output_path_str = output_path.unwrap_or("");
        match self
            .handle_completed_task_history(task_id, output_path_str, artifact_diagnostic)
            .await
        {
            Ok(TerminalHistoryRecordOutcome::Recorded(task)) => {
                self.mark_completed_child_exit_history_recorded(&task);
            }
            Ok(TerminalHistoryRecordOutcome::Ignored) => {}
            Err(err) => {
                let message = format!("任务已完成，但写入完成历史时出错：{}", err);
                self.mark_completed_child_exit_history_record_failed(task_id, &message);
            }
        }
    }

    fn mark_completed_child_exit_history_recorded(&self, task: &TaskSnapshot) {
        self.events
            .history_task_added(HistoryStatus::Completed, task);
    }

    fn mark_completed_child_exit_history_record_failed(&self, task_id: &str, message: &str) {
        self.diagnostics.warn(message);
        self.events.task_error(task_id, message);
    }

    async fn handle_completed_task_history(
        &self,
        task_id: &str,
        output_path: &str,
        artifact_diagnostic: Option<&crate::application::artifact_resolution::ArtifactDiagnostic>,
    ) -> AppResult<TerminalHistoryRecordOutcome> {
        handle_completed_task_history(
            &self.terminal_history_orchestrator(),
            task_id,
            output_path,
            artifact_diagnostic,
        )
        .await
    }

    fn current_settings(&self) -> AppSettings {
        self.settings_repository.get()
    }

    /// Returns whether auto-action on complete is enabled in current settings.
    /// Internal helper for shutdown intent method; not for direct external use.
    fn auto_action_on_complete_enabled(&self) -> bool {
        self.current_settings().auto_action_on_complete
    }

    fn countdown_start_decision(&self) -> ShutdownCountdownStartDecision {
        self.shutdown_scheduler.countdown_start_decision()
    }

    fn start_countdown(&self) -> AppResult<u64> {
        self.shutdown_scheduler.start_countdown()
    }

    fn decide_and_start_countdown(&self) -> AppResult<Option<u64>> {
        match self.countdown_start_decision() {
            ShutdownCountdownStartDecision::StartAllowed => {
                let seconds = self.start_countdown()?;
                Ok(Some(seconds))
            }
            ShutdownCountdownStartDecision::Blocked => Ok(None),
        }
    }

    /// High-level intent method for run-completion that handles shutdown countdown
    /// eligibility check, start decision, started-event emission, and failed-start warning.
    async fn handle_shutdown_countdown_after_finished_run(
        &self,
        exit_context: &str,
    ) -> AppResult<Option<u64>> {
        if !self.shutdown_countdown_requested_and_idle().await {
            return Ok(None);
        }

        match self.decide_and_start_countdown() {
            Ok(Some(seconds)) => {
                self.mark_shutdown_countdown_started(seconds);
                Ok(Some(seconds))
            }
            Ok(None) => Ok(None),
            Err(err) => {
                self.mark_shutdown_countdown_start_failed(exit_context, &err);
                Ok(None)
            }
        }
    }

    /// High-level intent method for run-completion to check if shutdown countdown
    /// should be considered after child exit. Returns true only when:
    /// - auto_action_on_complete is enabled in settings
    /// - there is no live work in the queue
    ///
    /// Preserves short-circuit behavior: does not query live work if auto_action is disabled.
    async fn shutdown_countdown_requested_and_idle(&self) -> bool {
        if !self.auto_action_on_complete_enabled() {
            return false;
        }
        !self.has_live_work().await
    }

    fn resolve_download_dir(&self, settings: &AppSettings) -> DownloadDirectory {
        self.download_directory_resolver
            .resolve_download_dir(settings)
    }

    fn current_download_dir(&self) -> DownloadDirectory {
        let settings = self.current_settings();
        self.resolve_download_dir(&settings)
    }

    /// Internal helper that schedules and starts the next task using current settings.
    /// Encapsulates: current_download_dir, try_schedule_next_with_dir.
    async fn try_schedule_next_and_start(&self) -> AppResult<ScheduleNextOutcome> {
        let download_dir = self.current_download_dir();
        self.try_schedule_next_with_dir(&download_dir).await
    }

    /// Internal queue-scheduler helper that drives the schedule-and-start loop
    /// for current settings while external callers use semantic intent methods.
    async fn schedule_next_internal(&self) -> AppResult<()> {
        self.try_schedule_next_and_start().await.map(|_| ())
    }

    async fn schedule_next_if_requested(&self, request: ScheduleNextRequest) -> AppResult<()> {
        match request {
            ScheduleNextRequest::Requested => self.schedule_next_internal().await,
            ScheduleNextRequest::NotRequested => Ok(()),
        }
    }

    /// Tail helper for queue-mutation scenarios (add / retry-from-history / retry-existing).
    /// Schedules next task if requested and emits queue-state-changed with scenario identity.
    /// Per ADR-0008: consolidates the 3 homogeneous `schedule_next + queue_state_changed` tails.
    async fn complete_queue_mutation_scheduling(
        &self,
        request: ScheduleNextRequest,
        scenario: QueueMutationScenario,
    ) -> AppResult<()> {
        self.schedule_next_if_requested(request).await?;
        scenario.emit_queue_changed(self.events);
        Ok(())
    }

    async fn add_task(&self, task: Task) -> AppResult<bool> {
        self.queue_repository.add_task(task).await
    }

    async fn retry_task(&self, task_id: &str) -> AppResult<TaskSnapshot> {
        self.queue_repository.retry_task(task_id).await
    }

    fn find_failed_history_task(&self, task_id: &str) -> AppResult<HistoryFindOutcome> {
        self.history_repository
            .find_task(HistoryStatus::Failed, task_id)
    }

    /// Queue-retry intent method that hides HistoryFindOutcome from callers.
    /// Returns the found TaskSnapshot or AppError::TaskNotFound for missing history.
    fn find_failed_history_task_or_not_found(&self, task_id: &str) -> AppResult<TaskSnapshot> {
        match self.find_failed_history_task(task_id)? {
            HistoryFindOutcome::Found(task) => Ok(task),
            HistoryFindOutcome::Missing => Err(AppError::TaskNotFound {
                id: task_id.to_string(),
            }),
        }
    }

    /// Internal helper that returns true only when there is live work.
    /// Calls queue repository directly; no longer delegates through run_ports.
    async fn has_live_work(&self) -> bool {
        self.queue_repository.live_work_status().await
    }

    /// Internal queue-start helper that returns true when the queue has no live work.
    async fn should_pause_queue_start(&self) -> bool {
        !self.has_live_work().await
    }

    pub(crate) async fn handle_queue_start(&self) -> AppResult<()> {
        let outcome = self.shutdown_scheduler.reset_for_new_run()?;
        match outcome {
            ShutdownResetOutcome::CountdownCancelled => {
                self.events.shutdown_countdown_cancelled();
            }
            ShutdownResetOutcome::NoCountdown => {}
        }
        self.drive_queue_start().await
    }

    async fn drive_queue_start(&self) -> AppResult<()> {
        if self.should_pause_queue_start().await {
            self.pause_run_queue_start().await?;
            return Ok(());
        }
        self.start_run_and_schedule_next_internal_queue_start()
            .await
    }

    async fn finish_run_if_idle(&self) -> AppResult<bool> {
        self.queue_repository.finish_run_if_idle().await
    }

    /// Internal helper that finishes run if idle and marks run-completion idle-finish if finished.
    /// Called only by finish_run_after_child_exit; not for direct external use.
    /// The run-completion idle-finish internal sequence owns this event marking.
    async fn finish_run_if_idle_and_mark(&self) -> AppResult<bool> {
        let outcome = self.finish_run_if_idle().await?;
        if outcome {
            self.mark_run_completion_idle_run_finished();
        }
        Ok(outcome)
    }

    fn mark_run_completion_idle_run_finished(&self) {
        self.events.queue_state_changed();
    }

    /// Internal run-status helper that pauses the queue run.
    /// Called only by pause_run_queue_start; not for direct external use.
    async fn pause_run(&self) -> AppResult<()> {
        self.queue_repository
            .set_run_status(QueueRunStatus::Paused)
            .await
    }

    /// Internal queue-start helper that pauses run and marks queue-start pause completion.
    /// Encapsulates: pause_run, queue-start pause event marking.
    async fn pause_run_queue_start(&self) -> AppResult<()> {
        self.pause_run().await?;
        self.mark_queue_start_paused();
        Ok(())
    }

    fn mark_queue_start_paused(&self) {
        self.events.queue_state_changed();
    }

    /// Internal run-status helper that starts the queue run.
    /// Called only by start_run_and_schedule_next_internal_queue_start; not for direct external use.
    async fn start_run(&self) -> AppResult<()> {
        self.queue_repository
            .set_run_status(QueueRunStatus::Running)
            .await
    }

    /// Intent method for queue-start to start run, schedule next for current settings,
    /// and mark queue-start run-start completion.
    /// Encapsulates: start_run, schedule_next_internal, queue-start run-start event marking.
    async fn start_run_and_schedule_next_internal_queue_start(&self) -> AppResult<()> {
        self.start_run().await?;
        self.schedule_next_internal().await?;
        self.mark_queue_start_run_started();
        Ok(())
    }

    fn mark_queue_start_run_started(&self) {
        self.events.queue_state_changed();
    }

    async fn schedule_next(&self) -> AppResult<Option<TaskSnapshot>> {
        self.queue_repository.schedule_next().await
    }

    /// Internal helper for task-lifecycle continuation intents. Shutdown acknowledgement
    /// is handled before completed/failed child-exit flows continue.
    async fn continue_child_exit_unless_shutting_down<F, Fut>(&self, continue_child_exit: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        if self.acknowledge_shutdown_child_exit_if_needed().await {
            return;
        }
        continue_child_exit().await;
    }

    /// High-level completed child-exit intent that owns terminal cleanup, history
    /// recording, run-completion continuation, and queue-state notification.
    ///
    /// Per ADR-0005 decision 5, artifact location is computed here (not by the
    /// adapter): the adapter only reports the raw facts (task_id + download_dir
    /// + save_name); this method snapshots the directory via `ArtifactInventory`
    /// and runs `locate_artifact` to resolve the artifact path.
    pub(crate) async fn handle_completed_child_exit(
        &self,
        task_id: &str,
        download_dir: &crate::application::artifact_inventory::ArtifactDir,
        save_name: Option<&str>,
    ) {
        let resolution = self.resolve_completed_artifact(download_dir, save_name).await;
        let output_path = match &resolution {
            crate::application::artifact_resolution::ArtifactResolution::Located(path) => {
                Some(path.as_str().to_string())
            }
            _ => None,
        };
        // Project the resolution's diagnostic onto TaskSnapshot (ADR-0005 stage 4.4 follow-up):
        // - Located → no diagnostic (artifact was found)
        // - NotFound → no diagnostic (normal "no match" outcome, not a failure worth recording)
        // - InventoryUnavailable → carry the diagnostic so history can explain why output_path is None
        let artifact_diagnostic = match &resolution {
            crate::application::artifact_resolution::ArtifactResolution::InventoryUnavailable(
                err,
            ) => Some(crate::application::artifact_resolution::ArtifactDiagnostic::from(err)),
            _ => None,
        };
        self.complete_child_exit(task_id, output_path.as_deref(), artifact_diagnostic)
            .await;
    }

    /// Resolve the artifact for a completed task. Returns the full
    /// `ArtifactResolution` so callers can distinguish:
    /// - `Located(path)` — artifact found, path persisted as `output_path`
    /// - `NotFound` — directory missing/empty, or no entry matched policy
    /// - `InventoryUnavailable(err)` — inventory IO failed; a warn is logged
    ///
    /// Keeping the full resolution (rather than collapsing to `Option<String>`)
    /// preserves the diagnostic for future projection onto `TaskSnapshot`.
    async fn resolve_completed_artifact(
        &self,
        download_dir: &crate::application::artifact_inventory::ArtifactDir,
        save_name: Option<&str>,
    ) -> crate::application::artifact_resolution::ArtifactResolution {
        use crate::application::artifact_location::{
            locate_artifact, ArtifactLocatePolicy, ArtifactLocateRequest,
        };
        use crate::application::artifact_inventory::InventoryMoment;
        use crate::application::artifact_resolution::ArtifactResolution;

        let snapshot = match self.artifact_inventory.snapshot(download_dir).await {
            Ok(s) => s,
            Err(err) => {
                self.diagnostics.warn(&format!(
                    "产物盘点失败，任务 {} 的产物路径将留空：{}",
                    save_name.unwrap_or("(no save_name)"),
                    err.message
                ));
                return ArtifactResolution::InventoryUnavailable(err);
            }
        };

        let now = InventoryMoment::new(self.clock.now());
        let policy = ArtifactLocatePolicy::default_for_n_m3u8dl_cli();
        let request = ArtifactLocateRequest::new(save_name.map(|s| s.to_string()));
        match locate_artifact(&snapshot, &request, &policy, now) {
            Some(path) => ArtifactResolution::Located(path),
            None => ArtifactResolution::NotFound,
        }
    }

    async fn complete_child_exit(
        &self,
        task_id: &str,
        output_path: Option<&str>,
        artifact_diagnostic: Option<crate::application::artifact_resolution::ArtifactDiagnostic>,
    ) {
        let output_path_owned = output_path.map(|s| s.to_string());
        self.clear_child_exit_terminal_active_line(task_id);
        self.continue_child_exit_unless_shutting_down(|| async {
            self.handle_completed_child_exit_history(
                task_id,
                output_path_owned.as_deref(),
                artifact_diagnostic.as_ref(),
            )
            .await;
            self.drive_child_exit_queue_and_handle_shutdown_countdown("completion")
                .await;
        })
        .await;
    }

    /// High-level failed child-exit intent that owns terminal cleanup, failure
    /// transition matching, run-completion continuation, and queue-state notification.
    pub(crate) async fn handle_failed_child_exit(&self, task_id: &str, error_message: &str) {
        self.fail_child_exit(task_id, error_message).await;
    }

    async fn fail_child_exit(&self, task_id: &str, error_message: &str) {
        self.clear_child_exit_terminal_active_line(task_id);
        self.continue_child_exit_unless_shutting_down(|| async {
            self.handle_failed_child_exit_internal(task_id, error_message)
                .await;
            self.drive_child_exit_queue_and_handle_shutdown_countdown("failure")
                .await;
        })
        .await;
    }

    async fn acknowledge_shutdown_child_exit_if_needed(&self) -> bool {
        if self.queue_is_shutting_down().await {
            self.mark_shutdown_child_exit_acknowledged();
            true
        } else {
            false
        }
    }

    fn mark_shutdown_child_exit_acknowledged(&self) {
        self.events.queue_state_changed();
    }

    async fn queue_is_shutting_down(&self) -> bool {
        self.queue_repository.shutdown_status().await
    }

    async fn process_is_shutting_down(&self) -> bool {
        match self.process_runner.shutdown_status().await {
            ProcessRunnerShutdownStatus::ShuttingDown => true,
            ProcessRunnerShutdownStatus::Running => false,
        }
    }

    async fn start_task(&self, request: TaskProcessStartRequest) -> AppResult<()> {
        self.process_runner.start_task(request).await
    }

    /// High-level intent method for run-completion to schedule next after child exit
    /// and mark the queue-change or failure outcome.
    /// Encapsulates: try_schedule_next_and_start, ScheduleNextOutcome matching,
    /// run-completion schedule-next event marking, run-completion schedule-next failure warning.
    async fn schedule_next_after_child_exit(&self, exit_context: &str) {
        match self.try_schedule_next_and_start().await {
            Ok(ScheduleNextOutcome::QueueChanged) => {
                self.mark_schedule_next_queue_changed_after_child_exit();
            }
            Ok(ScheduleNextOutcome::QueueUnchanged) => {}
            Err(err) => {
                self.mark_schedule_next_failed_after_child_exit(exit_context, &err);
            }
        }
    }

    fn mark_schedule_next_queue_changed_after_child_exit(&self) {
        self.events.queue_state_changed();
    }

    fn mark_schedule_next_failed_after_child_exit(
        &self,
        exit_context: &str,
        error: &dyn std::fmt::Display,
    ) {
        self.diagnostics.warn(&format!(
            "Failed to schedule next task after {}: {}",
            exit_context, error
        ));
    }

    /// High-level intent method for run-completion to directly record the run-completion
    /// shutdown countdown start failure warning. Encapsulates: direct diagnostics.warn emission.
    fn mark_shutdown_countdown_start_failed(
        &self,
        exit_context: &str,
        error: &dyn std::fmt::Display,
    ) {
        self.diagnostics.warn(&format!(
            "Failed to start shutdown countdown after {}: {}",
            exit_context, error
        ));
    }

    /// High-level intent method for run-completion to finish run after child exit
    /// and record the finished or failure outcome.
    /// Encapsulates: finish_run_if_idle_and_mark, bool outcome,
    /// run-completion finish-idle failure warning.
    /// Returns Some(outcome) on success, None on failure (after warning).
    async fn finish_run_after_child_exit(&self, exit_context: &str) -> Option<bool> {
        match self.finish_run_if_idle_and_mark().await {
            Ok(outcome) => Some(outcome),
            Err(err) => {
                self.mark_finish_idle_failed_after_child_exit(exit_context, &err);
                None
            }
        }
    }

    fn mark_finish_idle_failed_after_child_exit(
        &self,
        exit_context: &str,
        error: &dyn std::fmt::Display,
    ) {
        self.diagnostics.warn(&format!(
            "Failed to finish idle run after {}: {}",
            exit_context, error
        ));
    }

    /// Intent method for run-completion to finish run after child exit and report whether it finished.
    async fn finish_run_after_child_exit_and_report_finished(&self, exit_context: &str) -> bool {
        match self.finish_run_after_child_exit(exit_context).await {
            Some(true) => true,
            Some(false) | None => false,
        }
    }

    /// Intent method for run-completion to emit queue-state-changed after marking queue changed
    /// in the child-exit flow. Directly emits queue_state_changed event.
    fn mark_child_exit_queue_changed(&self) {
        self.events.queue_state_changed();
    }

    /// Intent method for run-completion to mark shutdown countdown started.
    /// Hides direct event emission behind run-completion-specific intent.
    fn mark_shutdown_countdown_started(&self, seconds: u64) {
        self.events.shutdown_countdown_started(seconds);
    }

    /// High-level intent method that drives the full child-exit queue-driving sequence
    /// for run-completion, then reports whether the run finished.
    /// Encapsulates: mark_child_exit_queue_changed, schedule_next_after_child_exit,
    /// finish_run_after_child_exit_and_report_finished.
    /// Returns true if the run finished, false otherwise.
    async fn drive_child_exit_queue_and_report_finished(&self, exit_context: &str) -> bool {
        self.mark_child_exit_queue_changed();
        self.schedule_next_after_child_exit(exit_context).await;
        self.finish_run_after_child_exit_and_report_finished(exit_context)
            .await
    }

    /// High-level run-completion intent that drives child-exit queue sequencing and
    /// handles shutdown countdown only when that sequencing finishes the run.
    async fn drive_child_exit_queue_and_handle_shutdown_countdown(&self, exit_context: &str) {
        let run_finished = self
            .drive_child_exit_queue_and_report_finished(exit_context)
            .await;
        if run_finished {
            self.handle_shutdown_countdown_after_finished_run(exit_context)
                .await
                .ok();
        }
    }

    /// Intent method for task-lifecycle to clear persisted and frontend active-line state
    /// after a child exit.
    fn clear_child_exit_terminal_active_line(&self, task_id: &str) {
        self.clear_persisted_child_exit_terminal_active_line(task_id);
        self.mark_child_exit_terminal_active_line_cleared(task_id);
    }

    fn clear_persisted_child_exit_terminal_active_line(&self, task_id: &str) {
        self.terminal_output_repository.clear_active_line(task_id);
    }

    fn mark_child_exit_terminal_active_line_cleared(&self, task_id: &str) {
        self.events.terminal_active_line(task_id, "");
    }

    /// High-level queue-add intent that owns task creation, repository add, scheduling,
    /// and queue-state notification.
    /// Returns TaskSnapshot derived from the queued task.
    pub(crate) async fn handle_queue_add(
        &self,
        task_creation_orchestrator: TaskCreationPorts<'_>,
        payload: AddTaskPayload,
    ) -> AppResult<TaskSnapshot> {
        self.add_new_task_queue_add(task_creation_orchestrator, payload)
            .await
    }

    async fn add_new_task_queue_add(
        &self,
        task_creation_orchestrator: TaskCreationPorts<'_>,
        payload: AddTaskPayload,
    ) -> AppResult<TaskSnapshot> {
        let task = self.create_queued_task(task_creation_orchestrator, payload);
        self.add_queue_add_task_and_schedule(task).await
    }

    fn create_queued_task(
        &self,
        task_creation_orchestrator: TaskCreationPorts<'_>,
        payload: AddTaskPayload,
    ) -> Task {
        task_creation_orchestrator.create_queued_task_from_payload(payload)
    }

    async fn add_queue_add_task_and_schedule(&self, task: Task) -> AppResult<TaskSnapshot> {
        let snapshot = TaskSnapshot::from(&task);
        let add_outcome = self.add_task(task).await?;
        self.complete_queue_mutation_scheduling(
            add_outcome.into(),
            QueueMutationScenario::QueueAdd,
        )
        .await?;
        Ok(snapshot)
    }

    /// High-level intent method for queue-retry fallback to add a history task and schedule next if requested.
    /// Encapsulates: add_task, complete_queue_mutation_scheduling (retry-from-history scenario).
    /// Returns TaskSnapshot derived from the queued task.
    async fn add_retry_history_task_queue_retry(&self, task: Task) -> AppResult<TaskSnapshot> {
        let snapshot = TaskSnapshot::from(&task);
        let add_outcome = self.add_task(task).await?;
        self.complete_queue_mutation_scheduling(
            add_outcome.into(),
            QueueMutationScenario::RetryFromHistory,
        )
        .await?;
        Ok(snapshot)
    }

    /// High-level intent method for queue-retry to retry an existing queue task and schedule next if requested.
    /// Encapsulates: retry_task, complete_queue_mutation_scheduling (retry-existing scenario).
    /// Returns TaskSnapshot from the retried task.
    async fn retry_existing_task_queue_retry(&self, task_id: &str) -> AppResult<TaskSnapshot> {
        let task = self.retry_task(task_id).await?;
        self.complete_queue_mutation_scheduling(
            ScheduleNextRequest::Requested,
            QueueMutationScenario::RetryExisting,
        )
        .await?;
        Ok(task)
    }

    /// High-level queue-retry intent that owns retry fallback, scheduling,
    /// and queue-state notification.
    pub(crate) async fn handle_queue_retry(
        &self,
        task_id: &str,
        task_creation_orchestrator: TaskCreationPorts<'_>,
    ) -> AppResult<TaskSnapshot> {
        self.retry_existing_or_restore_missing_queue_retry(task_id, task_creation_orchestrator)
            .await
    }

    async fn retry_existing_or_restore_missing_queue_retry(
        &self,
        task_id: &str,
        task_creation_orchestrator: TaskCreationPorts<'_>,
    ) -> AppResult<TaskSnapshot> {
        match self.retry_existing_task_queue_retry(task_id).await {
            Ok(task) => Ok(task),
            Err(AppError::TaskNotFound { .. }) => {
                self.restore_missing_queue_retry_from_history(task_id, task_creation_orchestrator)
                    .await
            }
            Err(err) => Err(err),
        }
    }

    async fn restore_missing_queue_retry_from_history(
        &self,
        task_id: &str,
        task_creation_orchestrator: TaskCreationPorts<'_>,
    ) -> AppResult<TaskSnapshot> {
        let history_task = self.find_failed_history_task_or_not_found(task_id)?;
        self.restore_failed_history_task_queue_retry(task_creation_orchestrator, &history_task)
            .await
    }

    async fn restore_failed_history_task_queue_retry(
        &self,
        task_creation_orchestrator: TaskCreationPorts<'_>,
        history_task: &TaskSnapshot,
    ) -> AppResult<TaskSnapshot> {
        let task =
            self.create_queued_task_from_failed_history(task_creation_orchestrator, history_task);
        self.add_retry_history_task_queue_retry(task).await
    }

    fn create_queued_task_from_failed_history(
        &self,
        task_creation_orchestrator: TaskCreationPorts<'_>,
        history_task: &TaskSnapshot,
    ) -> Task {
        task_creation_orchestrator.create_queued_task_from_history_retry(history_task)
    }

    /// Internal helper that drives the schedule-and-start loop for a resolved download directory.
    /// Encapsulates: schedule_next, build TaskProcessStartRequest, start_task,
    /// inspect shutdown gates after spawn failure, and handle_start_task_failure.
    async fn try_schedule_next_with_dir(
        &self,
        download_dir: &DownloadDirectory,
    ) -> AppResult<ScheduleNextOutcome> {
        let mut outcome = ScheduleNextOutcome::QueueUnchanged;
        loop {
            let task = match self.schedule_next().await? {
                Some(task) => {
                    outcome = ScheduleNextOutcome::QueueChanged;
                    task
                }
                None => {
                    return Ok(outcome);
                }
            };
            let task_id = task.id.clone();
            let request = TaskProcessStartRequest {
                task_id: task_id.clone(),
                url: task.url,
                save_name: task.save_name,
                headers: task.headers,
                download_dir: download_dir.clone(),
            };
            match self.start_task(request).await {
                Ok(()) => return Ok(ScheduleNextOutcome::QueueChanged),
                Err(err) => {
                    if self.process_is_shutting_down().await || self.queue_is_shutting_down().await
                    {
                        return Ok(outcome);
                    }
                    let error_message = err.to_string();
                    self.handle_start_task_failure(&task_id, &error_message)
                        .await;
                }
            }
        }
    }

    async fn handle_start_task_failure(&self, task_id: &str, error_message: &str) {
        self.mark_start_task_failed(task_id, error_message);
        match self
            .handle_start_failure_transition(task_id, error_message)
            .await
        {
            Ok(StartFailureOutcome::Terminal(task)) => {
                self.mark_start_task_failure_history_recorded(&task);
            }
            Ok(StartFailureOutcome::RetryScheduled) | Ok(StartFailureOutcome::Ignored) => {}
            Err(persist_err) => {
                let message = format!("任务启动失败，但写入失败历史时出错：{}", persist_err);
                self.mark_start_task_failure_persistence_failed(task_id, &message);
            }
        }
    }

    fn mark_start_task_failure_history_recorded(&self, task: &TaskSnapshot) {
        self.events.history_task_added(HistoryStatus::Failed, task);
    }

    fn mark_start_task_failed(&self, task_id: &str, error_message: &str) {
        self.diagnostics.warn(&format!(
            "Failed to start task {}: {}",
            task_id, error_message
        ));
    }

    fn mark_start_task_failure_persistence_failed(&self, task_id: &str, message: &str) {
        self.diagnostics.warn(message);
        self.events.task_error(task_id, message);
    }
}
