//! Interactive TUI mode for real-time effect processing.
//!
//! Provides a terminal-based interface for:
//! - Displaying the effect chain
//! - Editing effect parameters
//! - Toggling effect bypass
//! - Loading and saving presets

use clap::Args;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use sonido_config::{EffectConfig, Preset, factory_presets};
use std::io::stdout;
use std::time::Duration;

#[derive(Args)]
pub struct TuiArgs {
    /// Initial preset to load
    #[arg(short, long)]
    preset: Option<String>,
}

/// Application state for the TUI.
struct TuiApp {
    /// Current preset being edited.
    preset: Preset,
    /// Currently selected effect index.
    selected_effect: usize,
    /// Currently selected parameter index within the effect.
    selected_param: usize,
    /// Whether we're in parameter edit mode.
    editing: bool,
    /// Edit buffer for parameter values.
    edit_buffer: String,
    /// Status message to display.
    status: String,
    /// Whether the app should quit.
    should_quit: bool,
    /// List state for effect list.
    effect_list_state: ListState,
    /// List state for parameter list.
    param_list_state: ListState,
    /// Focus: 0 = effects list, 1 = params list.
    focus: usize,
}

impl TuiApp {
    fn new(preset: Preset) -> Self {
        let mut effect_list_state = ListState::default();
        effect_list_state.select(Some(0));

        let mut param_list_state = ListState::default();
        param_list_state.select(Some(0));

        Self {
            preset,
            selected_effect: 0,
            selected_param: 0,
            editing: false,
            edit_buffer: String::new(),
            status: "Press 'q' to quit, Tab to switch panels, Enter to edit".to_string(),
            should_quit: false,
            effect_list_state,
            param_list_state,
            focus: 0,
        }
    }

    fn current_effect(&self) -> Option<&EffectConfig> {
        self.preset.effects.get(self.selected_effect)
    }

    fn current_effect_mut(&mut self) -> Option<&mut EffectConfig> {
        self.preset.effects.get_mut(self.selected_effect)
    }

    fn handle_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) {
        if self.editing {
            self.handle_edit_key(key);
            return;
        }

        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => {
                self.focus = (self.focus + 1) % 2;
                self.update_list_states();
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Enter => self.start_editing(),
            KeyCode::Char(' ') => self.toggle_bypass(),
            KeyCode::Char('b') => self.toggle_bypass(),
            KeyCode::Char('r') => self.reset_param(),
            KeyCode::Left | KeyCode::Char('h') => {
                if self.focus == 1 {
                    self.adjust_param(-0.1);
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.focus == 1 {
                    self.adjust_param(0.1);
                }
            }
            KeyCode::Char('[') => self.adjust_param(-0.01),
            KeyCode::Char(']') => self.adjust_param(0.01),
            KeyCode::Char('{') => self.adjust_param(-1.0),
            KeyCode::Char('}') => self.adjust_param(1.0),
            _ => {}
        }
    }

    fn handle_edit_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Enter => self.confirm_edit(),
            KeyCode::Esc => self.cancel_edit(),
            KeyCode::Backspace => {
                self.edit_buffer.pop();
            }
            KeyCode::Char(c) => {
                if c.is_ascii_digit() || c == '.' || c == '-' {
                    self.edit_buffer.push(c);
                }
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: i32) {
        if self.focus == 0 {
            // Effects list
            let len = self.preset.effects.len();
            if len == 0 {
                return;
            }
            let new_idx = (self.selected_effect as i32 + delta).rem_euclid(len as i32) as usize;
            self.selected_effect = new_idx;
            self.selected_param = 0;
        } else {
            // Params list
            let param_count = self.current_effect().map(|e| e.params.len()).unwrap_or(0);
            if param_count == 0 {
                return;
            }
            let new_idx = (self.selected_param as i32 + delta).rem_euclid(param_count as i32) as usize;
            self.selected_param = new_idx;
        }
        self.update_list_states();
    }

    fn update_list_states(&mut self) {
        self.effect_list_state.select(Some(self.selected_effect));
        self.param_list_state.select(Some(self.selected_param));
    }

    fn start_editing(&mut self) {
        if self.focus == 1
            && let Some(effect) = self.current_effect() {
                let param_names: Vec<_> = effect.params.keys().collect();
                if let Some(param_name) = param_names.get(self.selected_param)
                    && let Some(value) = effect.params.get(*param_name) {
                        self.edit_buffer = value.clone();
                        self.editing = true;
                        self.status = "Editing: Enter to confirm, Esc to cancel".to_string();
                    }
            }
    }

    fn confirm_edit(&mut self) {
        let selected_param = self.selected_param;
        let edit_value = self.edit_buffer.clone();

        if let Some(effect) = self.preset.effects.get_mut(self.selected_effect) {
            let param_names: Vec<_> = effect.params.keys().cloned().collect();
            if let Some(param_name) = param_names.get(selected_param) {
                effect.params.insert(param_name.clone(), edit_value.clone());
                self.status = format!("Set {} = {}", param_name, edit_value);
            }
        }
        self.editing = false;
        self.edit_buffer.clear();
    }

    fn cancel_edit(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
        self.status = "Edit cancelled".to_string();
    }

    fn toggle_bypass(&mut self) {
        if let Some(effect) = self.current_effect_mut() {
            effect.bypassed = !effect.bypassed;
            let state = if effect.bypassed { "bypassed" } else { "active" };
            self.status = format!("{} is now {}", effect.effect_type, state);
        }
    }

    fn reset_param(&mut self) {
        self.status = "Reset parameter (not implemented)".to_string();
    }

    fn adjust_param(&mut self, delta: f32) {
        let selected_param = self.selected_param;
        let selected_effect = self.selected_effect;

        if let Some(effect) = self.preset.effects.get_mut(selected_effect) {
            let param_names: Vec<_> = effect.params.keys().cloned().collect();
            if let Some(param_name) = param_names.get(selected_param)
                && let Some(value_str) = effect.params.get(param_name).cloned()
                    && let Ok(value) = value_str.parse::<f32>() {
                        let new_value = value + delta;
                        effect.params.insert(param_name.clone(), format!("{:.2}", new_value));
                        self.status = format!("{} = {:.2}", param_name, new_value);
                    }
        }
    }
}

fn draw_ui(frame: &mut Frame, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Status bar
        ])
        .split(frame.area());

    draw_header(frame, chunks[0], app);
    draw_main(frame, chunks[1], app);
    draw_status(frame, chunks[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let title = format!(" SONIDO TUI - {} ", app.preset.name);
    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, area);
}

fn draw_main(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),  // Effects list
            Constraint::Percentage(65),  // Parameters panel
        ])
        .split(area);

    draw_effects_list(frame, chunks[0], app);
    draw_params_panel(frame, chunks[1], app);
}

fn draw_effects_list(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let items: Vec<ListItem> = app
        .preset
        .effects
        .iter()
        .enumerate()
        .map(|(i, effect)| {
            let bypass_marker = if effect.bypassed { " [OFF]" } else { "" };
            let text = format!("{}. {}{}", i + 1, effect.effect_type, bypass_marker);

            let style = if effect.bypassed {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Green)
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let block_style = if app.focus == 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Effects (Tab to switch) ")
                .borders(Borders::ALL)
                .border_style(block_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut app.effect_list_state);
}

fn draw_params_panel(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Effect name
            Constraint::Min(5),     // Parameters list
            Constraint::Length(5),  // Help
        ])
        .split(area);

    // Effect name header
    let effect_name = app
        .current_effect()
        .map(|e| e.effect_type.to_uppercase())
        .unwrap_or_else(|| "NO EFFECT".to_string());

    let header = Paragraph::new(effect_name)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).title(" Selected Effect "));
    frame.render_widget(header, chunks[0]);

    // Parameters list
    draw_params_list(frame, chunks[1], app);

    // Help text
    let help_text = if app.editing {
        "Type value, Enter to confirm, Esc to cancel"
    } else {
        "Space/b: bypass | h/l or [/]: adjust | Enter: edit | r: reset"
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title(" Controls "));
    frame.render_widget(help, chunks[2]);
}

fn draw_params_list(frame: &mut Frame, area: Rect, app: &mut TuiApp) {
    let items: Vec<ListItem> = if let Some(effect) = app.current_effect() {
        effect
            .params
            .iter()
            .enumerate()
            .map(|(i, (name, value))| {
                let display_value = if app.editing && i == app.selected_param && app.focus == 1 {
                    format!("{}_ ", app.edit_buffer)
                } else {
                    value.clone()
                };

                let text = format!("{:15} = {}", name, display_value);
                ListItem::new(text)
            })
            .collect()
    } else {
        vec![ListItem::new("(no parameters)")]
    };

    let block_style = if app.focus == 1 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Parameters ")
                .borders(Borders::ALL)
                .border_style(block_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut app.param_list_state);
}

fn draw_status(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let status = Paragraph::new(app.status.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(" Status "));
    frame.render_widget(status, area);
}

pub fn run(args: TuiArgs) -> anyhow::Result<()> {
    // Load initial preset
    let preset = if let Some(preset_name) = &args.preset {
        sonido_config::get_factory_preset(preset_name)
            .or_else(|| {
                sonido_config::find_preset(preset_name)
                    .and_then(|path| Preset::load(&path).ok())
            })
            .ok_or_else(|| anyhow::anyhow!("Preset '{}' not found", preset_name))?
    } else {
        // Default to init preset
        factory_presets()
            .into_iter()
            .next()
            .unwrap_or_else(|| Preset::new("Untitled"))
    };

    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new(preset);

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut TuiApp,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press {
                    app.handle_key(key.code, key.modifiers);
                }

        if app.should_quit {
            return Ok(());
        }
    }
}
