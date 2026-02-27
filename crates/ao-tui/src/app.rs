use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use ao_core::models::{AgentStatus, RepoCandidate, Slot, SlotStatus};
use ao_core::services::log_tailer::LogSource as CoreLogSource;

/// The active mode determines which UI is shown and how keys are dispatched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    SlotList,
    CreateSlotDialog,
    SpawnAgentDialog,
    ConfirmDialog {
        message: String,
        action: ConfirmAction,
    },
    HelpDialog,
    Loading(String),
    MultiplexLog,
    BlueprintListDialog,
    BlueprintSaveDialog,
    BatchProgress,
}

/// What a confirmed dialog action should do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    DestroySlot(String),
    DestroyAll,
    LoadBlueprint(String),
    Quit,
}

/// Which log source to display for the selected slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    Agent,
    Aspire,
}

/// Which view is active: the normal slot list or the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    SlotList,
    Dashboard,
}

// ─── Dashboard Activity Tracking ───────────────────────────────────────

/// Activity metrics for a single slot, computed from log output.
pub struct SlotActivity {
    pub log_timestamps: VecDeque<Instant>,
    pub sparkline_data: Vec<u64>,
    pub last_log_line: Option<String>,
    pub needs_attention: bool,
    pub attention_reason: Option<String>,
}

impl Default for SlotActivity {
    fn default() -> Self {
        Self {
            log_timestamps: VecDeque::new(),
            sparkline_data: vec![0; 20],
            last_log_line: None,
            needs_attention: false,
            attention_reason: None,
        }
    }
}

// ─── Log Multiplexer ───────────────────────────────────────────────────

/// Severity level heuristically determined from log text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warn,
    Info,
    Debug,
}

/// A single entry in the multiplexed log buffer.
pub struct LogEntry {
    pub slot_name: String,
    pub source: CoreLogSource,
    pub text: String,
    pub severity: Severity,
    pub color_index: u8,
}

/// Ring buffer holding interleaved log entries from all slots.
pub struct LogBuffer {
    pub entries: VecDeque<LogEntry>,
    pub slot_colors: HashMap<String, u8>,
    next_color: u8,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(Self::CAPACITY),
            slot_colors: HashMap::new(),
            next_color: 0,
        }
    }
}

impl LogBuffer {
    const CAPACITY: usize = 10_000;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, slot_name: String, source: CoreLogSource, text: String) {
        let color_index = self.color_for_slot(&slot_name);
        let severity = classify_severity(&text);

        if self.entries.len() >= Self::CAPACITY {
            self.entries.pop_front();
        }

        self.entries.push_back(LogEntry {
            slot_name,
            source,
            text,
            severity,
            color_index,
        });
    }

    fn color_for_slot(&mut self, name: &str) -> u8 {
        if let Some(&idx) = self.slot_colors.get(name) {
            return idx;
        }
        let idx = self.next_color % 8;
        self.next_color += 1;
        self.slot_colors.insert(name.to_string(), idx);
        idx
    }
}

/// Heuristic severity classification from log text.
fn classify_severity(text: &str) -> Severity {
    let lower = text.to_lowercase();
    if lower.contains("error") || lower.contains("exception") || lower.contains("fatal") {
        Severity::Error
    } else if lower.contains("warn") {
        Severity::Warn
    } else if lower.contains("debug") || lower.contains("trace") {
        Severity::Debug
    } else {
        Severity::Info
    }
}

/// Filter state for the multiplexed log view.
#[derive(Default)]
pub struct MultiplexFilter {
    pub hidden_slots: HashSet<String>,
    pub source_filter: Option<CoreLogSource>,
    pub search_text: String,
    pub search_regex: Option<regex::Regex>,
    pub search_filter_mode: bool,
    pub search_input_active: bool,
}

impl MultiplexFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_regex(&mut self) {
        if self.search_text.is_empty() {
            self.search_regex = None;
        } else {
            self.search_regex = regex::Regex::new(&regex::escape(&self.search_text)).ok();
        }
    }

    pub fn cycle_source(&mut self) {
        self.source_filter = match self.source_filter {
            None => Some(CoreLogSource::Agent),
            Some(CoreLogSource::Agent) => Some(CoreLogSource::Aspire),
            Some(CoreLogSource::Aspire) => None,
        };
    }

    pub fn matches_entry(&self, entry: &LogEntry) -> bool {
        if self.hidden_slots.contains(&entry.slot_name) {
            return false;
        }
        if let Some(src) = self.source_filter {
            if entry.source != src {
                return false;
            }
        }
        if self.search_filter_mode {
            if let Some(ref re) = self.search_regex {
                if !re.is_match(&entry.text) {
                    return false;
                }
            }
        }
        true
    }
}

// ─── Blueprint & Batch ─────────────────────────────────────────────────

/// State for the blueprint list dialog.
pub struct BlueprintListState {
    pub names: Vec<String>,
    pub selected: usize,
    pub loading: bool,
}

impl Default for BlueprintListState {
    fn default() -> Self {
        Self {
            names: Vec::new(),
            selected: 0,
            loading: true,
        }
    }
}

impl BlueprintListState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// State for the blueprint save dialog.
pub struct BlueprintSaveState {
    pub name: String,
    pub description: String,
    pub focus: BlueprintSaveField,
}

impl Default for BlueprintSaveState {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            focus: BlueprintSaveField::Name,
        }
    }
}

impl BlueprintSaveState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BlueprintSaveField {
    Name,
    Description,
}

/// Progress state for a running batch operation.
pub struct BatchProgressState {
    pub title: String,
    pub total: usize,
    pub completed: usize,
    pub current_slot: Option<String>,
    pub failures: Vec<(String, String)>,
    pub done: bool,
}

impl BatchProgressState {
    pub fn new(title: &str, total: usize) -> Self {
        Self {
            title: title.to_string(),
            total,
            completed: 0,
            current_slot: None,
            failures: Vec::new(),
            done: false,
        }
    }
}

// ─── Forms ─────────────────────────────────────────────────────────────

/// Form state for the create-slot dialog.
#[derive(Debug)]
pub struct CreateSlotForm {
    pub source: String,
    pub prompt: String,
    pub focus: CreateSlotField,
    pub all_candidates: Vec<RepoCandidate>,
    pub filtered_candidates: Vec<RepoCandidate>,
    pub selected_candidate: Option<usize>,
    pub scan_loading: bool,
    pub filter_deadline: Option<Instant>,
}

impl Default for CreateSlotForm {
    fn default() -> Self {
        Self {
            source: String::new(),
            prompt: String::new(),
            focus: CreateSlotField::default(),
            all_candidates: Vec::new(),
            filtered_candidates: Vec::new(),
            selected_candidate: None,
            scan_loading: true,
            filter_deadline: None,
        }
    }
}

impl CreateSlotForm {
    /// Run fuzzy matching on all_candidates using self.source as query.
    pub fn apply_filter(&mut self) {
        let query = self.source.trim();
        if query.is_empty() {
            self.filtered_candidates = self.all_candidates.iter().take(6).cloned().collect();
        } else {
            let matcher = SkimMatcherV2::default();
            let mut scored: Vec<(i64, &RepoCandidate)> = self
                .all_candidates
                .iter()
                .filter_map(|c| {
                    let name_score = matcher.fuzzy_match(&c.name, query).unwrap_or(0);
                    let path_score = c
                        .local_path
                        .as_deref()
                        .and_then(|p| matcher.fuzzy_match(p, query))
                        .unwrap_or(0);
                    let best = name_score.max(path_score);
                    if best > 0 {
                        Some((best, c))
                    } else {
                        None
                    }
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            self.filtered_candidates = scored.into_iter().take(6).map(|(_, c)| c.clone()).collect();
        }

        self.selected_candidate = if self.filtered_candidates.is_empty() {
            None
        } else {
            Some(0)
        };
        self.filter_deadline = None;
    }

    pub fn schedule_filter(&mut self) {
        self.filter_deadline = Some(Instant::now() + std::time::Duration::from_millis(400));
    }

    pub fn should_filter_now(&self) -> bool {
        self.filter_deadline
            .map(|d| Instant::now() >= d)
            .unwrap_or(false)
    }

    pub fn select_next_candidate(&mut self) {
        if let Some(idx) = self.selected_candidate {
            if idx + 1 < self.filtered_candidates.len() {
                self.selected_candidate = Some(idx + 1);
            }
        }
    }

    pub fn select_prev_candidate(&mut self) {
        if let Some(idx) = self.selected_candidate {
            if idx > 0 {
                self.selected_candidate = Some(idx - 1);
            }
        }
    }

    pub fn accept_selected(&mut self) -> bool {
        if let Some(idx) = self.selected_candidate {
            if let Some(candidate) = self.filtered_candidates.get(idx) {
                self.source = candidate.source_value().to_string();
                self.filtered_candidates.clear();
                self.selected_candidate = None;
                self.filter_deadline = None;
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum CreateSlotField {
    #[default]
    Source,
    Prompt,
}

/// Form state for the spawn-agent dialog.
#[derive(Debug)]
pub struct SpawnAgentForm {
    pub prompt: String,
    pub allowed_tools: String,
    pub max_turns: String,
    pub focus: SpawnAgentField,
}

impl Default for SpawnAgentForm {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            allowed_tools: "Bash,Read,Glob,Grep,Write,Edit,WebFetch,WebSearch,Task".into(),
            max_turns: String::new(),
            focus: SpawnAgentField::Prompt,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum SpawnAgentField {
    #[default]
    Prompt,
    AllowedTools,
    MaxTurns,
}

// ─── Main App State ────────────────────────────────────────────────────

/// Top-level application state.
pub struct App {
    pub mode: Mode,
    pub view: ViewMode,
    pub slots: Vec<Slot>,
    pub selected_index: usize,
    pub log_source: LogSource,
    pub log_lines: Vec<String>,
    pub log_scroll: usize,
    pub log_auto_follow: bool,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub create_form: CreateSlotForm,
    pub agent_form: SpawnAgentForm,
    /// Set by pop-in key handler; consumed by main loop.
    pub pop_in_target: Option<String>,

    // Dashboard
    pub activity: HashMap<String, SlotActivity>,
    pub dashboard_selected: usize,

    // Log Multiplexer
    pub log_buffer: LogBuffer,
    pub multiplex_filter: MultiplexFilter,
    pub multiplex_scroll: usize,
    pub multiplex_auto_follow: bool,

    // Blueprint
    pub blueprint_list: BlueprintListState,
    pub blueprint_save: BlueprintSaveState,

    // Batch
    pub batch_progress: Option<BatchProgressState>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            mode: Mode::SlotList,
            view: ViewMode::SlotList,
            slots: Vec::new(),
            selected_index: 0,
            log_source: LogSource::Agent,
            log_lines: Vec::new(),
            log_scroll: 0,
            log_auto_follow: true,
            should_quit: false,
            status_message: None,
            create_form: CreateSlotForm::default(),
            agent_form: SpawnAgentForm::default(),
            pop_in_target: None,
            activity: HashMap::new(),
            dashboard_selected: 0,
            log_buffer: LogBuffer::new(),
            multiplex_filter: MultiplexFilter::new(),
            multiplex_scroll: 0,
            multiplex_auto_follow: true,
            blueprint_list: BlueprintListState::new(),
            blueprint_save: BlueprintSaveState::new(),
            batch_progress: None,
        }
    }

    pub fn selected_slot(&self) -> Option<&Slot> {
        self.slots.get(self.selected_index)
    }

    pub fn select_next(&mut self) {
        if !self.slots.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.slots.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.slots.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.slots.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    pub fn toggle_log_source(&mut self) {
        self.log_source = match self.log_source {
            LogSource::Agent => LogSource::Aspire,
            LogSource::Aspire => LogSource::Agent,
        };
        self.log_lines.clear();
        self.log_scroll = 0;
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    /// Record a log line for activity tracking (dashboard sparklines).
    pub fn record_activity(&mut self, slot_name: &str, line: &str) {
        let activity = self.activity.entry(slot_name.to_string()).or_default();

        activity.log_timestamps.push_back(Instant::now());
        if activity.log_timestamps.len() > 120 {
            activity.log_timestamps.pop_front();
        }

        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let truncated = if trimmed.chars().count() > 60 {
                let end: String = trimmed.chars().take(57).collect();
                format!("{end}...")
            } else {
                trimmed.to_string()
            };
            activity.last_log_line = Some(truncated);
        }
    }

    /// Recompute sparkline data and attention flags for all slots.
    pub fn recompute_activity(&mut self) {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(120); // 2 minute window
        let bucket_size = std::time::Duration::from_secs(6); // 20 buckets of 6s each

        for slot in &self.slots {
            let activity = self.activity.entry(slot.name.clone()).or_default();

            // Prune timestamps older than window
            while activity
                .log_timestamps
                .front()
                .is_some_and(|t| now.duration_since(*t) > window)
            {
                activity.log_timestamps.pop_front();
            }

            // Bucket into 20 sparkline values
            let mut buckets = [0u64; 20];
            for ts in &activity.log_timestamps {
                let age = now.duration_since(*ts);
                if age <= window {
                    let bucket_idx = (age.as_secs() / bucket_size.as_secs()) as usize;
                    if bucket_idx < 20 {
                        // Reverse: most recent = rightmost
                        buckets[19 - bucket_idx] += 1;
                    }
                }
            }
            // Normalize to 0-8
            let max_val = buckets.iter().copied().max().unwrap_or(1).max(1);
            activity.sparkline_data = buckets.iter().map(|&v| (v * 8) / max_val).collect();

            // Evaluate attention flags
            activity.needs_attention = false;
            activity.attention_reason = None;

            if slot.agent_status == AgentStatus::Blocked {
                activity.needs_attention = true;
                activity.attention_reason = Some("Agent blocked".to_string());
            } else if slot.status == SlotStatus::Error {
                activity.needs_attention = true;
                activity.attention_reason = Some("Aspire error".to_string());
            } else if slot.agent_status == AgentStatus::Active {
                // Check idle time
                if let Some(last_ts) = activity.log_timestamps.back() {
                    let idle_secs = now.duration_since(*last_ts).as_secs();
                    if idle_secs >= 300 {
                        activity.needs_attention = true;
                        let mins = idle_secs / 60;
                        activity.attention_reason = Some(format!("Idle {mins}m"));
                    }
                }
            }
        }
    }

    /// Navigate dashboard grid.
    pub fn dashboard_move(&mut self, dx: i32, dy: i32) {
        if self.slots.is_empty() {
            return;
        }
        let cols = self.dashboard_columns();
        let current = self.dashboard_selected;
        let row = current / cols;
        let col = current % cols;

        let new_col = (col as i32 + dx).clamp(0, cols as i32 - 1) as usize;
        let new_row = (row as i32 + dy).max(0) as usize;
        let new_idx = new_row * cols + new_col;

        if new_idx < self.slots.len() {
            self.dashboard_selected = new_idx;
        }
    }

    /// Calculate number of dashboard columns based on slot count.
    pub fn dashboard_columns(&self) -> usize {
        let count = self.slots.len();
        if count <= 4 {
            2
        } else {
            3
        }
    }
}
