use eframe::egui;
use std::collections::HashMap;
use egui_material_icons as icons;

#[derive(Clone)]
struct Task {
    id: usize,
    text: String,
    completed: bool,
}

#[derive(Clone)]
struct Project {
    id: usize,
    name: String,
    tasks: Vec<Task>,
    expanded: bool,
}

struct TodoApp {
    projects: Vec<Project>,
    next_project_id: usize,
    next_task_id: usize,
    new_project_name: String,
    editing_project: Option<usize>,
    editing_task: Option<(usize, usize)>, // (project_id, task_id)
    new_task_texts: HashMap<usize, String>, // project_id -> new task text
    edit_project_text: String,
    edit_task_text: String,
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
        }
    }
}

impl TodoApp {}

impl eframe::App for TodoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Fixed font sizes
            let heading_size = 24.0;
            let label_size = 16.0;
            let button_size = 14.0;
            let text_size = 14.0;

            ui.heading(egui::RichText::new("Todo App").size(heading_size));
            ui.separator();

            // Add new project section
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("New Project:").size(label_size));
                let response = ui.text_edit_singleline(&mut self.new_project_name);

                if ui.button(
                    egui::RichText::new(format!("{} Add Project", icons::icons::ICON_ADD))
                        .size(button_size)
                ).clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    self.add_project();
                }
            });

            ui.separator();

            // Display projects
            let mut projects_to_remove = Vec::new();
            let mut project_actions = Vec::new(); // Store actions to perform after iteration
            let mut task_actions = Vec::new(); // Store task actions


            for (project_idx, project) in self.projects.iter_mut().enumerate() {
                ui.group(|ui| {
                    // Project header
                    ui.horizontal(|ui| {
                        // Expand/collapse button
                        let expand_icon = if project.expanded {
                            icons::icons::ICON_EXPAND_MORE
                        } else {
                            icons::icons::ICON_CHEVRON_RIGHT
                        };
                        if ui.button(
                            egui::RichText::new(expand_icon).size(button_size)
                        ).clicked() {
                            project.expanded = !project.expanded;
                        }

                        // Project name and controls
                        if self.editing_project == Some(project.id) {
                            // Editing mode: show text input with confirmation buttons
                            let response = ui.text_edit_singleline(&mut self.edit_project_text);
                            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                if !self.edit_project_text.trim().is_empty() {
                                    project.name = self.edit_project_text.clone();
                                }
                                project_actions.push(("stop_edit", project.id, String::new()));
                            } else if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                project_actions.push(("stop_edit", project.id, String::new()));
                            }

                            if ui.button(icons::icons::ICON_CHECK).clicked() {
                                if !self.edit_project_text.trim().is_empty() {
                                    project.name = self.edit_project_text.clone();
                                }
                                project_actions.push(("stop_edit", project.id, String::new()));
                            }
                            if ui.button(icons::icons::ICON_CLOSE).clicked() {
                                project_actions.push(("stop_edit", project.id, String::new()));
                            }
                        } else {
                            // Display mode: show label with edit button
                            ui.label(egui::RichText::new(&project.name).size(label_size));
                            if ui.button(
                                egui::RichText::new(icons::icons::ICON_EDIT).size(button_size)
                            ).clicked() {
                                project_actions.push(("start_edit", project.id, project.name.clone()));
                            }
                        }

                        // Delete project button
                        if ui.button(
                            egui::RichText::new(icons::icons::ICON_DELETE).size(button_size)
                        ).clicked() {
                            projects_to_remove.push(project_idx);
                        }
                    });

                    // Tasks (only shown when expanded)
                    if project.expanded {
                        ui.indent("tasks", |ui| {
                            let mut tasks_to_remove = Vec::new();

                            for (task_idx, task) in project.tasks.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    // Checkbox for completion
                                    ui.checkbox(&mut task.completed, "");

                                    // Task text and controls
                                    if self.editing_task == Some((project.id, task.id)) {
                                        // Editing mode: show text input with confirmation buttons
                                        let response = ui.text_edit_singleline(&mut self.edit_task_text);
                                        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            if !self.edit_task_text.trim().is_empty() {
                                                task.text = self.edit_task_text.clone();
                                            }
                                            task_actions.push(("stop_edit", project.id, task.id, String::new()));
                                        } else if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                            task_actions.push(("stop_edit", project.id, task.id, String::new()));
                                        }

                                        if ui.button(icons::icons::ICON_CHECK).clicked() {
                                            if !self.edit_task_text.trim().is_empty() {
                                                task.text = self.edit_task_text.clone();
                                            }
                                            task_actions.push(("stop_edit", project.id, task.id, String::new()));
                                        }
                                        if ui.button(icons::icons::ICON_CLOSE).clicked() {
                                            task_actions.push(("stop_edit", project.id, task.id, String::new()));
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
                                            egui::RichText::new(&task.text).size(text_size)
                                        );
                                        if ui.button(icons::icons::ICON_EDIT).clicked() {
                                            task_actions.push(("start_edit", project.id, task.id, task.text.clone()));
                                        }
                                    }

                                    // Delete task button
                                    if ui.button(icons::icons::ICON_DELETE).clicked() {
                                        tasks_to_remove.push(task_idx);
                                    }
                                });
                            }

                            // Remove tasks
                            for &idx in tasks_to_remove.iter().rev() {
                                project.tasks.remove(idx);
                            }

                            // Add new task
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("New Task:").size(label_size));
                                let task_text = self.new_task_texts.entry(project.id).or_insert_with(String::new);
                                let response = ui.text_edit_singleline(task_text);

                                if ui.button(
                                    egui::RichText::new(format!("{} Add Task", icons::icons::ICON_ADD))
                                        .size(button_size)
                                ).clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                                    task_actions.push(("add", project.id, 0, String::new()));
                                }
                            });
                        });
                    }
                });
                ui.add_space(8.0);
            }

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
                    "add" => {
                        self.add_task_to_project(project_id);
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

    fn add_task_to_project(&mut self, project_id: usize) {
        if let Some(task_text) = self.new_task_texts.get(&project_id) {
            if !task_text.trim().is_empty() {
                let task = Task {
                    id: self.next_task_id,
                    text: task_text.clone(),
                    completed: false,
                };

                if let Some(project) = self.projects.iter_mut().find(|p| p.id == project_id) {
                    project.tasks.push(task);
                    self.next_task_id += 1;
                    self.new_task_texts.insert(project_id, String::new());
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
            Ok(Box::new(TodoApp::default()))
        }),
    )
}
