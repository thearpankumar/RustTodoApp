use chrono::prelude::*;
use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_material_icons as icons;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};

// Notes Canvas Data Structures
#[derive(Clone, Serialize, Deserialize)]
struct TextBox {
    id: usize,
    title: String,
    position: egui::Pos2,
    size: egui::Vec2,

    // New Markdown content
    #[serde(default)]
    content: String,

    #[serde(default = "default_auto_height")]
    auto_height: bool,

    min_size: egui::Vec2,
    #[serde(skip)]
    is_dragging: bool,
}

fn default_auto_height() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct NotesCanvas {
    text_boxes: Vec<TextBox>,
    next_textbox_id: usize,
    scene_rect: egui::Rect,
}

impl Default for NotesCanvas {
    fn default() -> Self {
        Self {
            text_boxes: Vec::new(),
            next_textbox_id: 1,
            scene_rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(5000.0, 5000.0)),
        }
    }
}

// Todo App Data Structures
#[derive(Clone, Serialize, Deserialize)]
struct Task {
    id: usize,
    text: String,
    completed: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct Project {
    id: usize,
    name: String,
    tasks: Vec<Task>,
    expanded: bool,
}

#[derive(Serialize, Deserialize)]
struct TodoApp {
    projects: Vec<Project>,
    next_project_id: usize,
    next_task_id: usize,
    #[serde(skip)]
    new_project_name: String,
    #[serde(skip)]
    editing_project: Option<usize>,
    #[serde(skip)]
    editing_task: Option<(usize, usize)>, // (project_id, task_id)
    #[serde(skip)]
    new_task_texts: HashMap<usize, String>, // project_id -> new task text
    #[serde(skip)]
    edit_project_text: String,
    #[serde(skip)]
    edit_task_text: String,
    #[serde(skip)]
    adding_task_to_project: Option<usize>, // Project ID for right-click task creation
    #[serde(skip)]
    right_click_task_text: HashMap<usize, String>, // Task text for each project's right-click creation
    // Notes canvas fields
    notes_canvas: NotesCanvas,
    #[serde(skip)]
    show_notes: bool,
    #[serde(skip)]
    context_menu_pos: Option<egui::Pos2>,

    // Editing state
    #[serde(skip)]
    editing_textbox: Option<usize>, // ID of textbox being edited
    #[serde(skip)]
    commonmark_cache: CommonMarkCache, // Cache for markdown rendering

    #[serde(skip)]
    editing_title: Option<usize>, // ID of textbox whose title is being edited
    #[serde(skip)]
    temp_title_text: String,
}

impl Default for TodoApp {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            next_project_id: 1,
            next_task_id: 1,
            new_project_name: String::new(),
            editing_project: None,
            editing_task: None,
            new_task_texts: HashMap::new(),
            edit_project_text: String::new(),
            edit_task_text: String::new(),
            adding_task_to_project: None,
            right_click_task_text: HashMap::new(),
            notes_canvas: NotesCanvas::default(),
            show_notes: false,
            context_menu_pos: None,
            editing_textbox: None,
            commonmark_cache: CommonMarkCache::default(),
            editing_title: None,
            temp_title_text: String::new(),
        }
    }
}

impl TodoApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load data from storage if available

        // Customize fonts

        // --- Persistence Loading Strategy ---
        // Release: Use eframe's default storage (OS standard paths)
        // Debug: Use local file "todo_data.json" in CWD

        let loaded_app: Option<Self> = if cfg!(debug_assertions) {
            // Debug Mode: Try loading from local file
            if let Ok(file) = File::open("todo_data.json") {
                let reader = BufReader::new(file);
                serde_json::from_reader(reader).ok()
            } else {
                None
            }
        } else {
            // Release Mode: Use eframe storage
            if let Some(storage) = cc.storage {
                eframe::get_value(storage, eframe::APP_KEY)
            } else {
                None
            }
        };

        if let Some(mut app) = loaded_app {
            // Restore transient/runtime state
            app.commonmark_cache = CommonMarkCache::default();
            app.editing_textbox = None;
            app.editing_title = None;
            app.adding_task_to_project = None;
            app.editing_task = None;
            app.right_click_task_text = HashMap::new();
            app.new_task_texts = HashMap::new();
            app.context_menu_pos = None;
            app.temp_title_text = String::new();
            app.edit_task_text = String::new();

            // Ensure auto_height is set correctly for old data if needed (though serde default handles it)
            // Fix text boxes that might have come from older saves without auto_height
            // (already handled by serde default)

            return app;
        }

        // If load failed or no save exists, return default
        Default::default()
    }
}

impl eframe::App for TodoApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // --- Persistence Saving Strategy ---
        // Release: Use eframe's default storage
        // Debug: Use local file "todo_data.json"

        if cfg!(debug_assertions) {
            // Debug Mode: Save to local file
            if let Ok(file) = File::create("todo_data.json") {
                let writer = BufWriter::new(file);
                // Pretty print for easier debugging
                let _ = serde_json::to_writer_pretty(writer, self);
            }
        } else {
            // Release Mode: Save to eframe storage
            eframe::set_value(storage, eframe::APP_KEY, self);
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Render floating pen button (always visible)
        self.render_floating_pen_button(ctx);

        // Conditionally render main content
        if self.show_notes {
            self.render_notes_canvas(ctx);
        } else {
            self.render_todo_view(ctx);
        }
    }
}

impl TodoApp {
    fn render_floating_pen_button(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("floating_pen_button"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-20.0, -20.0))
            .show(ctx, |ui| {
                let button_size = 60.0;
                let pen_icon = if self.show_notes {
                    icons::icons::ICON_CLOSE // X to close notes
                } else {
                    icons::icons::ICON_EDIT // Pen to open notes
                };

                if ui
                    .add_sized(
                        [button_size, button_size],
                        egui::Button::new(egui::RichText::new(pen_icon).size(24.0))
                            .corner_radius(30.0),
                    )
                    .clicked()
                {
                    self.show_notes = !self.show_notes;
                }
            });
    }

    fn render_todo_view(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Fixed font sizes
            let heading_size = 24.0;
            let project_title_size = 20.0; // Larger font for project titles
            let label_size = 16.0;
            let button_size = 14.0;
            let text_size = 16.0; // Increased task text size for better visibility

            ui.horizontal(|ui| {
                // Left side - Username
                ui.label(
                    egui::RichText::new(format!(
                        "User: {}",
                        whoami::username().unwrap_or_else(|_| "Unknown".to_string())
                    ))
                    .size(label_size),
                );

                // Get the remaining width for the rest of the layout
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Right side - Current date with padding (from right to left)
                    ui.add_space(10.0); // Padding from right edge
                    let now = Local::now();
                    ui.label(
                        egui::RichText::new(format!("{}", now.format("%d/%m/%Y"))).size(label_size),
                    );

                    // Center the title in remaining space
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            ui.heading(egui::RichText::new("Todo App").size(heading_size));
                        },
                    );
                });
            });
            ui.separator();

            // Add new project section
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("New Project:").size(label_size));
                let response = ui.text_edit_singleline(&mut self.new_project_name);

                if ui
                    .button(
                        egui::RichText::new(format!("{} Add Project", icons::icons::ICON_ADD))
                            .size(button_size),
                    )
                    .clicked()
                    || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    self.add_project();
                }
            });

            ui.add_space(16.0);

            // Simplified instruction for users
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Right-click on the expand/collapse button ({}/{}) to add tasks directly!",
                        icons::icons::ICON_CHEVRON_RIGHT,
                        icons::icons::ICON_EXPAND_MORE
                    ))
                    .size(label_size)
                    .color(egui::Color32::GRAY),
                );
            });

            ui.add_space(16.0);

            ui.separator();

            // Display projects in a scroll area
            let (projects_to_remove, project_actions, task_actions) = egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let mut projects_to_remove = Vec::new();
                    let mut project_actions = Vec::new(); // Store actions to perform after iteration
                    let mut task_actions = Vec::new(); // Store task actions

                    for (project_idx, project) in self.projects.iter_mut().enumerate() {
                        ui.push_id(project.id, |ui| {
                            let _frame_response = egui::Frame::group(ui.style())
                                .inner_margin(egui::Margin::same(16))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    // Project header
                                    ui.horizontal(|ui| {
                                        // Expand/collapse button with right-click to add task
                                        let expand_icon = if project.expanded {
                                            icons::icons::ICON_EXPAND_MORE
                                        } else {
                                            icons::icons::ICON_CHEVRON_RIGHT
                                        };
                                        let expand_response = ui.button(
                                            egui::RichText::new(expand_icon).size(button_size),
                                        );

                                        if expand_response.clicked() {
                                            project.expanded = !project.expanded;
                                        }

                                        // Right-click on expand button to add task
                                        if expand_response.secondary_clicked() {
                                            project_actions.push((
                                                "add_task",
                                                project.id,
                                                String::new(),
                                            ));
                                        }

                                        // Project name and controls
                                        if self.editing_project == Some(project.id) {
                                            // Editing mode: show text input with confirmation buttons
                                            let response = ui
                                                .text_edit_singleline(&mut self.edit_project_text);
                                            if response.lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                            {
                                                if !self.edit_project_text.trim().is_empty() {
                                                    project.name = self.edit_project_text.clone();
                                                }
                                                project_actions.push((
                                                    "stop_edit",
                                                    project.id,
                                                    String::new(),
                                                ));
                                            } else if response.lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Escape))
                                            {
                                                project_actions.push((
                                                    "stop_edit",
                                                    project.id,
                                                    String::new(),
                                                ));
                                            }

                                            if ui.button(icons::icons::ICON_CHECK).clicked() {
                                                if !self.edit_project_text.trim().is_empty() {
                                                    project.name = self.edit_project_text.clone();
                                                }
                                                project_actions.push((
                                                    "stop_edit",
                                                    project.id,
                                                    String::new(),
                                                ));
                                            }
                                            if ui.button(icons::icons::ICON_CLOSE).clicked() {
                                                project_actions.push((
                                                    "stop_edit",
                                                    project.id,
                                                    String::new(),
                                                ));
                                            }
                                        } else {
                                            // Display mode: show label with edit button
                                            ui.label(
                                                egui::RichText::new(&project.name)
                                                    .size(project_title_size),
                                            );

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    // Delete project button
                                                    if ui
                                                        .button(
                                                            egui::RichText::new(
                                                                icons::icons::ICON_DELETE,
                                                            )
                                                            .size(button_size),
                                                        )
                                                        .clicked()
                                                    {
                                                        projects_to_remove.push(project_idx);
                                                    }

                                                    // Edit project button
                                                    if ui
                                                        .button(
                                                            egui::RichText::new(
                                                                icons::icons::ICON_EDIT,
                                                            )
                                                            .size(button_size),
                                                        )
                                                        .clicked()
                                                    {
                                                        project_actions.push((
                                                            "start_edit",
                                                            project.id,
                                                            project.name.clone(),
                                                        ));
                                                    }
                                                },
                                            );
                                        }
                                    });

                                    // Tasks (only shown when expanded)
                                    if project.expanded {
                                        ui.indent("tasks", |ui| {
                                            let mut tasks_to_remove = Vec::new();

                                            for (task_idx, task) in
                                                project.tasks.iter_mut().enumerate()
                                            {
                                                ui.add_space(8.0);
                                                ui.horizontal(|ui| {
                                                    // Checkbox for completion
                                                    ui.checkbox(&mut task.completed, "");

                                                    // Task text and controls
                                                    if self.editing_task
                                                        == Some((project.id, task.id))
                                                    {
                                                        // Editing mode: show text input with confirmation buttons
                                                        let response = ui.text_edit_singleline(
                                                            &mut self.edit_task_text,
                                                        );
                                                        if response.lost_focus()
                                                            && ui.input(|i| {
                                                                i.key_pressed(egui::Key::Enter)
                                                            })
                                                        {
                                                            if !self
                                                                .edit_task_text
                                                                .trim()
                                                                .is_empty()
                                                            {
                                                                task.text =
                                                                    self.edit_task_text.clone();
                                                            }
                                                            task_actions.push((
                                                                "stop_edit",
                                                                project.id,
                                                                task.id,
                                                                String::new(),
                                                            ));
                                                        } else if response.lost_focus()
                                                            && ui.input(|i| {
                                                                i.key_pressed(egui::Key::Escape)
                                                            })
                                                        {
                                                            task_actions.push((
                                                                "stop_edit",
                                                                project.id,
                                                                task.id,
                                                                String::new(),
                                                            ));
                                                        }

                                                        if ui
                                                            .button(icons::icons::ICON_CHECK)
                                                            .clicked()
                                                        {
                                                            if !self
                                                                .edit_task_text
                                                                .trim()
                                                                .is_empty()
                                                            {
                                                                task.text =
                                                                    self.edit_task_text.clone();
                                                            }
                                                            task_actions.push((
                                                                "stop_edit",
                                                                project.id,
                                                                task.id,
                                                                String::new(),
                                                            ));
                                                        }
                                                        if ui
                                                            .button(icons::icons::ICON_CLOSE)
                                                            .clicked()
                                                        {
                                                            task_actions.push((
                                                                "stop_edit",
                                                                project.id,
                                                                task.id,
                                                                String::new(),
                                                            ));
                                                        }
                                                    } else {
                                                        // Display mode: show label with edit button
                                                        let text_color = if task.completed {
                                                            ui.visuals().weak_text_color()
                                                        } else {
                                                            ui.visuals().text_color()
                                                        };
                                                        ui.colored_label(
                                                            text_color,
                                                            egui::RichText::new(&task.text)
                                                                .size(text_size),
                                                        );

                                                        ui.with_layout(
                                                            egui::Layout::right_to_left(
                                                                egui::Align::Center,
                                                            ),
                                                            |ui| {
                                                                // Delete task button
                                                                if ui
                                                                    .button(
                                                                        icons::icons::ICON_DELETE,
                                                                    )
                                                                    .clicked()
                                                                {
                                                                    tasks_to_remove.push(task_idx);
                                                                }

                                                                // Edit task button
                                                                if ui
                                                                    .button(icons::icons::ICON_EDIT)
                                                                    .clicked()
                                                                {
                                                                    task_actions.push((
                                                                        "start_edit",
                                                                        project.id,
                                                                        task.id,
                                                                        task.text.clone(),
                                                                    ));
                                                                }
                                                            },
                                                        );
                                                    }
                                                });
                                            }

                                            // Remove tasks
                                            for &idx in tasks_to_remove.iter().rev() {
                                                project.tasks.remove(idx);
                                            }

                                            // Show inline task creation UI when this project is selected for task addition
                                            if self.adding_task_to_project == Some(project.id) {
                                                ui.add_space(8.0);
                                                ui.horizontal(|ui| {
                                                    ui.label("New Task:");
                                                    let task_text = self
                                                        .right_click_task_text
                                                        .get_mut(&project.id)
                                                        .unwrap();
                                                    let response =
                                                        ui.text_edit_singleline(task_text);

                                                    if ui.button(icons::icons::ICON_CHECK).clicked()
                                                        || (response.lost_focus()
                                                            && ui.input(|i| {
                                                                i.key_pressed(egui::Key::Enter)
                                                            }))
                                                    {
                                                        if !task_text.trim().is_empty() {
                                                            project_actions.push((
                                                                "create_task",
                                                                project.id,
                                                                task_text.clone(),
                                                            ));
                                                        }
                                                        project_actions.push((
                                                            "cancel_add_task",
                                                            project.id,
                                                            String::new(),
                                                        ));
                                                    }

                                                    if ui.button(icons::icons::ICON_CLOSE).clicked()
                                                        || (response.lost_focus()
                                                            && ui.input(|i| {
                                                                i.key_pressed(egui::Key::Escape)
                                                            }))
                                                    {
                                                        project_actions.push((
                                                            "cancel_add_task",
                                                            project.id,
                                                            String::new(),
                                                        ));
                                                    }
                                                });
                                            }
                                        });
                                    }
                                });
                        });
                        ui.add_space(16.0);
                    }

                    (projects_to_remove, project_actions, task_actions)
                })
                .inner;

            // Process project actions
            for (action, project_id, text) in project_actions {
                match action {
                    "start_edit" => {
                        self.editing_project = Some(project_id);
                        self.edit_project_text = text;
                    }
                    "stop_edit" => {
                        self.editing_project = None;
                    }
                    "add_task" => {
                        self.adding_task_to_project = Some(project_id);
                        // Initialize the text field for this project if it doesn't exist
                        self.right_click_task_text.entry(project_id).or_default();
                    }
                    "create_task" => {
                        self.add_task_to_project(project_id, text);
                    }
                    "cancel_add_task" => {
                        self.adding_task_to_project = None;
                        if let Some(task_text) = self.right_click_task_text.get_mut(&project_id) {
                            task_text.clear();
                        }
                    }
                    _ => {}
                }
            }

            // Process task actions
            for (action, project_id, task_id, text) in task_actions {
                match action {
                    "start_edit" => {
                        self.editing_task = Some((project_id, task_id));
                        self.edit_task_text = text;
                    }
                    "stop_edit" => {
                        self.editing_task = None;
                    }
                    _ => {}
                }
            }

            // Remove projects
            for &idx in projects_to_remove.iter().rev() {
                let project_id = self.projects[idx].id;
                self.projects.remove(idx);
                self.new_task_texts.remove(&project_id);
            }
        });
    }

    fn render_notes_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let rect = ui.max_rect();

            // Detect middle mouse or two-finger drag for panning
            let canvas_response = ui.interact(
                rect,
                egui::Id::new("canvas_interact"),
                egui::Sense::click_and_drag(),
            );

            // Pan with middle mouse button
            if canvas_response.dragged_by(egui::PointerButton::Middle) {
                self.notes_canvas.scene_rect = self
                    .notes_canvas
                    .scene_rect
                    .translate(canvas_response.drag_delta());
            }

            // Pan with two-finger trackpad drag
            // translation_delta() provides panning from scrolling or pan gestures
            let pan_delta = ui.input(|i| i.translation_delta());
            if pan_delta != egui::Vec2::ZERO {
                self.notes_canvas.scene_rect = self.notes_canvas.scene_rect.translate(pan_delta);
            }

            // Render background with pan offset
            self.render_canvas_background(ui);

            // Render text boxes with pan offset
            self.render_text_boxes(ui);

            // Handle right-click context menu
            self.handle_context_menu(ui);
        });

        // Center button in top-right corner
        egui::Area::new(egui::Id::new("center_button"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-20.0, 20.0))
            .show(ctx, |ui| {
                let button_size = 40.0;
                if ui
                    .add_sized(
                        [button_size, button_size],
                        egui::Button::new(
                            egui::RichText::new(icons::icons::ICON_CENTER_FOCUS_STRONG).size(20.0),
                        )
                        .corner_radius(5.0),
                    )
                    .on_hover_text("Reset view to center")
                    .clicked()
                {
                    // Reset to center
                    self.notes_canvas.scene_rect =
                        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(5000.0, 5000.0));
                }
            });

        // Render editing dialog (on top of canvas)
        // self.render_editing_dialog(ctx); // Removed

        // Render formatting dialog (on top of canvas)
        // self.render_formatting_dialog(ctx); // Removed
    }

    fn render_canvas_background(&self, ui: &mut egui::Ui) {
        let painter = ui.painter();
        let rect = ui.max_rect();
        let pan_offset = self.notes_canvas.scene_rect.min.to_vec2();

        // Background - Black (fill entire visible rect)
        painter.rect_filled(rect, 0.0, egui::Color32::BLACK);

        // Grid lines - Dark gray
        let grid_spacing = 50.0;
        let grid_color = egui::Color32::from_gray(50);

        // Calculate grid positions with pan offset
        let start_x = ((rect.min.x - pan_offset.x) / grid_spacing).floor() * grid_spacing;
        let start_y = ((rect.min.y - pan_offset.y) / grid_spacing).floor() * grid_spacing;

        // Vertical lines
        let mut x = start_x;
        while x < rect.max.x - pan_offset.x + grid_spacing {
            let screen_x = x + pan_offset.x;
            if screen_x >= rect.min.x && screen_x <= rect.max.x {
                painter.line_segment(
                    [
                        egui::pos2(screen_x, rect.min.y),
                        egui::pos2(screen_x, rect.max.y),
                    ],
                    egui::Stroke::new(1.0, grid_color),
                );
            }
            x += grid_spacing;
        }

        // Horizontal lines
        let mut y = start_y;
        while y < rect.max.y - pan_offset.y + grid_spacing {
            let screen_y = y + pan_offset.y;
            if screen_y >= rect.min.y && screen_y <= rect.max.y {
                painter.line_segment(
                    [
                        egui::pos2(rect.min.x, screen_y),
                        egui::pos2(rect.max.x, screen_y),
                    ],
                    egui::Stroke::new(1.0, grid_color),
                );
            }
            y += grid_spacing;
        }

        // Show helper text if no text boxes (at viewport center)
        if self.notes_canvas.text_boxes.is_empty() {
            let center = rect.center();
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                "Right-click anywhere to create a text box\nMiddle-click or two-finger drag to pan",
                egui::FontId::proportional(24.0),
                egui::Color32::WHITE,
            );
        }
    }

    fn render_text_boxes(&mut self, ui: &mut egui::Ui) {
        let mut actions: Vec<(&str, usize)> = Vec::new(); // (action, textbox_id)
        let pan_offset = self.notes_canvas.scene_rect.min.to_vec2();

        // Iterate through text boxes (render in order for z-ordering)
        for text_box_idx in 0..self.notes_canvas.text_boxes.len() {
            let text_box = &mut self.notes_canvas.text_boxes[text_box_idx];
            let id = egui::Id::new("textbox").with(text_box.id);

            // Apply pan offset to text box position
            let screen_position = text_box.position + pan_offset;

            // Calculate frame rect
            // If auto-height, we use a flexible height initially (Infinity) so it can grow
            // If manual, we use the stored size
            let current_size = if text_box.auto_height {
                egui::vec2(text_box.size.x, f32::INFINITY)
            } else {
                text_box.size
            };

            let frame_rect = egui::Rect::from_min_size(screen_position, current_size);
            let textbox_id = text_box.id;

            // Create a child UI at the text box position
            // We use a builder but we DON'T strictly force max_rect if we want auto-growth
            // However, for the frame background to be drawn correctly, we wrap it.
            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(frame_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );

            let frame = egui::Frame::group(child_ui.style())
                .fill(egui::Color32::from_gray(30))
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_gray(80)));

            let inner_response = frame.show(&mut child_ui, |ui| {
                ui.set_width(text_box.size.x - 20.0);

                // If MANUAL mode, force the frame to be at least the user-defined height
                if !text_box.auto_height {
                    ui.set_min_height(text_box.size.y - 20.0); // -20 for padding/margins approximation
                }

                // Header with title and delete button
                // Use a Right-to-Left layout to position the delete button first (on the right)
                // Then use the remaining space for the Title, making it the drag handle.
                let mut delta = egui::Vec2::ZERO;

                // Allocate a fixed height strip for the header so it doesn't float in the middle
                // of the box due to Align::Center
                let header_height = 30.0;
                let header_rect = ui
                    .allocate_exact_size(
                        egui::vec2(ui.available_width(), header_height),
                        egui::Sense::hover(),
                    )
                    .0;

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(header_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // 1. Delete Button (Right aligned)
                            if ui
                                .button(egui::RichText::new(icons::icons::ICON_DELETE).size(14.0))
                                .clicked()
                            {
                                actions.push(("delete", textbox_id));
                            }

                            // 2. Title (Fills remaining space to the left)
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    // Check if this title is being edited
                                    if self.editing_title == Some(textbox_id) {
                                        let response = ui.add_sized(
                                            egui::vec2(ui.available_width(), 24.0),
                                            egui::TextEdit::singleline(&mut self.temp_title_text),
                                        );

                                        if response.lost_focus() {
                                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                actions.push(("save_title", textbox_id));
                                            } else if ui.input(|i| i.key_pressed(egui::Key::Escape))
                                            {
                                                actions.push(("cancel_title_edit", textbox_id));
                                            }
                                        }
                                    } else {
                                        // Display Title - Make it draggable and double-clickable
                                        let title_response = ui.add_sized(
                                            egui::vec2(ui.available_width(), 24.0),
                                            egui::Label::new(
                                                egui::RichText::new(&text_box.title)
                                                    .strong()
                                                    .size(16.0)
                                                    .color(egui::Color32::from_gray(200)),
                                            )
                                            .sense(egui::Sense::click_and_drag()),
                                        );

                                        if title_response.double_clicked() {
                                            actions.push(("edit_title", textbox_id));
                                        }

                                        if title_response.dragged_by(egui::PointerButton::Primary) {
                                            delta = title_response.drag_delta();
                                        }
                                    }
                                },
                            );
                        });
                    },
                );

                ui.separator();

                // Main Content Area
                let is_editing = self.editing_textbox == Some(textbox_id);

                if is_editing {
                    // Edit Mode
                    // Use ui.add with desired_width instead of add_sized to allow height to grow naturally
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut text_box.content)
                            .frame(false)
                            .desired_width(ui.available_width()),
                    );

                    if response.clicked_elsewhere() {
                        actions.push(("stop_editing", textbox_id));
                    }
                } else {
                    // View Mode
                    let scroll_area =
                        egui::ScrollArea::vertical().id_salt(format!("scroll_{}", textbox_id));

                    // In auto-height mode, we don't want the scroll area to shrink or scroll, just expand.
                    // In manual mode, we want it to scroll if content is too big.
                    let scroll_area = if text_box.auto_height {
                        scroll_area.auto_shrink([false, true]) // Let it grow vertically
                    } else {
                        scroll_area.auto_shrink([false, false]) // Fill available space
                    };

                    let scroll_response = scroll_area.show(ui, |ui| {
                        if text_box.content.is_empty() {
                            if ui
                                .add(
                                    egui::Label::new("Double-click to edit...")
                                        .sense(egui::Sense::click()),
                                )
                                .double_clicked()
                            {
                                actions.push(("start_editing", textbox_id));
                            }
                        } else {
                            CommonMarkViewer::new().show(
                                ui,
                                &mut self.commonmark_cache,
                                &text_box.content,
                            );
                        }
                    });

                    if !text_box.content.is_empty()
                        && ui
                            .interact(
                                scroll_response.inner_rect,
                                id.with("view_interact"),
                                egui::Sense::click(),
                            )
                            .double_clicked()
                    {
                        actions.push(("start_editing", textbox_id));
                    }
                }

                delta
            });

            let header_drag_delta = inner_response.inner;
            let response = inner_response.response;

            // Handle dragging (Position Update)
            let text_box = &mut self.notes_canvas.text_boxes[text_box_idx];
            // Only update position if we got a delta from the header
            if header_drag_delta != egui::Vec2::ZERO {
                text_box.is_dragging = true;
                text_box.position += header_drag_delta;
            } else {
                text_box.is_dragging = false;
            }

            // Note: We removed the old drag logic associated with 'response' (the whole frame)

            // If we are in auto_height mode, update the size to match the frame size
            if text_box.auto_height {
                // FIXED: Only update HEIGHT. Preserve width to prevent shrinking loop!
                text_box.size.y = response.rect.height();
            }

            // Resize handle
            let resize_handle_size = 15.0;
            // The handle should ALWAYS be at the bottom-right of the actual rendered frame
            let resize_handle_pos = egui::pos2(
                response.rect.max.x - resize_handle_size,
                response.rect.max.y - resize_handle_size,
            );
            let resize_handle_rect = egui::Rect::from_min_size(
                resize_handle_pos,
                egui::vec2(resize_handle_size, resize_handle_size),
            );

            let resize_response = ui.interact(
                resize_handle_rect,
                id.with("resize_handle"),
                egui::Sense::drag(),
            );

            // Double-click resize handle to reset to auto-height
            // double_clicked() defaults to primary button
            if resize_response.double_clicked() {
                text_box.auto_height = true;
            }

            if resize_response.dragged_by(egui::PointerButton::Primary) {
                let delta = resize_response.drag_delta();

                // If dragging height, disable auto_height
                if delta.y.abs() > 0.0 {
                    text_box.auto_height = false;
                }

                text_box.size += delta;
                text_box.size = text_box.size.max(text_box.min_size);
            }

            // Visual feedback for resize handle
            let resize_color = if resize_response.hovered() {
                egui::Color32::from_gray(100)
            } else {
                egui::Color32::from_gray(180)
            };

            // Draw handle ONLY if the frame is visible/rendered
            ui.painter()
                .rect_filled(resize_handle_rect, 2.0, resize_color);
        }

        // Handle clicking background to stop editing
        if ui.input(|i| i.pointer.primary_clicked()) {
            // If we clicked, and it wasn't handled by a specific box, check where it was
            // This is a bit tricky in immediate mode.
            // A simple heuristic: if we are editing, and we click something that ISN'T the edit box, stop editing.
            // The `response.clicked_elsewhere()` covers most cases, but global click handling helps too.
        }

        // Process actions
        for (action, textbox_id) in actions {
            match action {
                "delete" => {
                    self.notes_canvas
                        .text_boxes
                        .retain(|tb| tb.id != textbox_id);
                }
                "edit_title" => {
                    if let Some(textbox) = self
                        .notes_canvas
                        .text_boxes
                        .iter()
                        .find(|tb| tb.id == textbox_id)
                    {
                        self.editing_title = Some(textbox_id);
                        self.temp_title_text = textbox.title.clone();
                    }
                }
                "save_title" => {
                    if let Some(textbox) = self
                        .notes_canvas
                        .text_boxes
                        .iter_mut()
                        .find(|tb| tb.id == textbox_id)
                    {
                        if !self.temp_title_text.trim().is_empty() {
                            textbox.title = self.temp_title_text.clone();
                        }
                    }
                    self.editing_title = None;
                    self.temp_title_text.clear();
                }
                "cancel_title_edit" => {
                    self.editing_title = None;
                    self.temp_title_text.clear();
                }
                "start_editing" => {
                    self.editing_textbox = Some(textbox_id);
                }
                "stop_editing" => {
                    self.editing_textbox = None;
                }
                _ => {}
            }
        }
    }

    fn handle_context_menu(&mut self, ui: &mut egui::Ui) {
        let pan_offset = self.notes_canvas.scene_rect.min.to_vec2();

        // Detect right-click
        ui.input(|i| {
            if i.pointer.secondary_clicked() {
                if let Some(screen_pos) = i.pointer.interact_pos() {
                    // Convert screen position to canvas position
                    let canvas_pos = screen_pos - pan_offset;

                    // Check if click is not on any text box
                    let on_textbox = self.notes_canvas.text_boxes.iter().any(|tb| {
                        egui::Rect::from_min_size(tb.position, tb.size).contains(canvas_pos)
                    });

                    if !on_textbox {
                        self.context_menu_pos = Some(screen_pos);
                    }
                }
            }
        });

        // Show context menu
        if let Some(menu_screen_pos) = self.context_menu_pos {
            egui::Area::new(egui::Id::new("context_menu"))
                .fixed_pos(menu_screen_pos)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        if ui.button("Create Text Box").clicked() {
                            let canvas_pos = menu_screen_pos - pan_offset;
                            self.create_text_box_at(canvas_pos);
                            self.context_menu_pos = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.context_menu_pos = None;
                        }
                    });
                });
        }

        // Close menu on click elsewhere
        ui.input(|i| {
            if i.pointer.primary_clicked() && self.context_menu_pos.is_some() {
                self.context_menu_pos = None;
            }
        });
    }

    fn create_text_box_at(&mut self, pos: egui::Pos2) {
        let text_box = TextBox {
            id: self.notes_canvas.next_textbox_id,
            title: format!("Note {}", self.notes_canvas.next_textbox_id),
            position: pos,
            size: egui::vec2(400.0, 250.0), // Fixed default size
            content: String::new(),
            auto_height: false, // Start in manual mode as requested
            min_size: egui::vec2(150.0, 80.0),
            is_dragging: false,
        };

        self.notes_canvas.text_boxes.push(text_box);
        self.notes_canvas.next_textbox_id += 1;
    }

    // Todo methods
    fn add_project(&mut self) {
        if !self.new_project_name.trim().is_empty() {
            let project = Project {
                id: self.next_project_id,
                name: self.new_project_name.clone(),
                tasks: Vec::new(),
                expanded: true,
            };
            self.projects.push(project);
            self.next_project_id += 1;
            self.new_project_name.clear();
        }
    }

    fn add_task_to_project(&mut self, project_id: usize, task_text: String) {
        if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
            if !task_text.trim().is_empty() {
                let task = Task {
                    id: self.next_task_id,
                    text: task_text.trim().to_string(),
                    completed: false,
                };
                project.tasks.push(task);
                self.next_task_id += 1;
            }
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Todo App",
        options,
        Box::new(|cc| {
            // Initialize the material icons - this sets up the icon fonts
            egui_material_icons::initialize(&cc.egui_ctx);

            // Don't override fonts at all to preserve material icons
            Ok(Box::new(TodoApp::new(cc)))
        }),
    )
}
