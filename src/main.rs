use chrono::prelude::*;
use eframe::egui;
use egui_material_icons as icons;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    adding_task_mode: bool, // Whether we're in task creation mode
    #[serde(skip)]
    selected_project_for_task: Option<usize>, // Which project to add task to
    #[serde(skip)]
    new_task_text: String, // Text for the new task being created
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
            adding_task_mode: false,
            selected_project_for_task: None,
            new_task_text: String::new(),
        }
    }
}

impl TodoApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load data from storage if available
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, "todo_app_data").unwrap_or_default();
        }
        Default::default()
    }
}

impl eframe::App for TodoApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "todo_app_data", self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                    egui::RichText::new(format!("User: {}", whoami::username()))
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
                    ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
                        ui.heading(egui::RichText::new("Todo App").size(heading_size));
                    });
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

            // Global Add Task button
            ui.horizontal(|ui| {
                if ui
                    .button(
                        egui::RichText::new(format!("{} Add Task", icons::icons::ICON_ADD))
                            .size(button_size),
                    )
                    .clicked()
                {
                    self.adding_task_mode = !self.adding_task_mode;
                    if !self.adding_task_mode {
                        // Reset when canceling
                        self.selected_project_for_task = None;
                        self.new_task_text.clear();
                    }
                }

                if self.adding_task_mode {
                    ui.label("Cancel with button again");
                }
            });

            // Task creation UI (only shown when in adding_task_mode)
            if self.adding_task_mode {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Select Project:").size(label_size));

                    // Project selection dropdown
                    egui::ComboBox::from_id_salt("project_selector")
                        .selected_text(if let Some(selected_id) = self.selected_project_for_task {
                            self.projects
                                .iter()
                                .find(|p| p.id == selected_id)
                                .map(|p| p.name.as_str())
                                .unwrap_or("Select Project")
                        } else {
                            "Select Project"
                        })
                        .show_ui(ui, |ui| {
                            for project in &self.projects {
                                ui.selectable_value(
                                    &mut self.selected_project_for_task,
                                    Some(project.id),
                                    &project.name,
                                );
                            }
                        });
                });

                // Task input field (only shown when project is selected)
                if self.selected_project_for_task.is_some() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Task Text:").size(label_size));
                        let response = ui.text_edit_singleline(&mut self.new_task_text);

                        if ui
                            .button(egui::RichText::new("Create Task").size(button_size))
                            .clicked()
                            || (response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            self.create_new_task();
                        }
                    });
                }
            }

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
                            egui::Frame::group(ui.style())
                                .inner_margin(egui::Margin::same(16))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    // Project header
                                    ui.horizontal(|ui| {
                                        // Expand/collapse button
                                        let expand_icon = if project.expanded {
                                            icons::icons::ICON_EXPAND_MORE
                                        } else {
                                            icons::icons::ICON_CHEVRON_RIGHT
                                        };
                                        if ui
                                            .button(
                                                egui::RichText::new(expand_icon).size(button_size),
                                            )
                                            .clicked()
                                        {
                                            project.expanded = !project.expanded;
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
}

impl TodoApp {
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

    fn create_new_task(&mut self) {
        if let Some(project_id) = self.selected_project_for_task {
            if !self.new_task_text.trim().is_empty() {
                let task = Task {
                    id: self.next_task_id,
                    text: self.new_task_text.clone(),
                    completed: false,
                };

                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    project.tasks.push(task);
                    self.next_task_id += 1;

                    // Reset the task creation state
                    self.new_task_text.clear();
                    self.adding_task_mode = false;
                    self.selected_project_for_task = None;
                }
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
