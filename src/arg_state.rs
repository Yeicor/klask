use crate::{error::ValidationErrorInfoTrait, KlaskUi, ValidationErrorInfo};
use clap::{Arg, ArgSettings, ValueHint};
use eframe::egui::{ComboBox, Ui};
use inflector::Inflector;
use native_dialog::FileDialog;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ArgState {
    pub name: String,
    pub call_name: Option<String>,
    pub desc: Option<String>,
    pub optional: bool,
    pub use_equals: bool,
    pub kind: ArgKind,
}

#[derive(Debug, Clone)]
pub enum ArgKind {
    String {
        value: String,
        default: Option<String>,
    },
    MultipleStrings {
        values: Vec<String>,
        default: Vec<String>,
    },
    Occurences(i32),
    Bool(bool),
    Path {
        value: String,
        default: Option<String>,
        allow_dir: bool,
        allow_file: bool,
    },
    MultiplePaths {
        values: Vec<String>,
        default: Vec<String>,
        allow_dir: bool,
        allow_file: bool,
    },
    Choose {
        value: ChooseState,
        possible: Vec<String>,
    },
    MultipleChoose {
        values: Vec<ChooseState>,
        possible: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ChooseState(pub String, pub Uuid);

impl Default for ChooseState {
    fn default() -> Self {
        Self(Default::default(), Uuid::new_v4())
    }
}

impl ArgState {
    pub fn update(&mut self, ui: &mut Ui, validation_error: &mut Option<ValidationErrorInfo>) {
        let label = ui.label(&self.name);

        if let Some(desc) = &self.desc {
            label.on_hover_text(desc);
        }

        // Not needed in edition 2021 with new closure borrowing rules
        let ArgState {
            name,
            optional,
            kind,
            ..
        } = self;

        match kind {
            ArgKind::String { value, default } => {
                ui.error_style_if(
                    (!*optional && value.is_empty()) || validation_error.is(name).is_some(),
                    |ui| {
                        let text = ui.text_edit_singleline_hint(
                            value,
                            match (default, *optional) {
                                (Some(default), _) => default.as_str(),
                                (_, true) => "(Optional)",
                                (_, false) => "",
                            },
                        );

                        if let Some(message) = validation_error.is(name) {
                            if text.on_hover_text(message).changed() {
                                *validation_error = None;
                            }
                        }
                    },
                );
            }
            ArgKind::Occurences(i) => {
                let list = ui.horizontal(|ui| {
                    if ui.small_button("-").clicked() {
                        *i = (*i - 1).max(0);
                    }

                    ui.error_style_if(validation_error.is(name).is_some(), |ui| {
                        ui.label(i.to_string());
                    });

                    if ui.small_button("+").clicked() {
                        *i += 1;
                    }
                });

                if let Some(message) = validation_error.is(name) {
                    if list.response.on_hover_text(message).changed() {
                        *validation_error = None;
                    }
                }
            }
            ArgKind::Bool(bool) => {
                ui.checkbox(bool, "");
            }
            ArgKind::MultipleStrings { values, default } => {
                ui.multiple_values(
                    validation_error,
                    name,
                    values,
                    Some(default),
                    |ui, value| ui.text_edit_singleline(value),
                );
            }
            ArgKind::Path {
                value,
                default,
                allow_dir,
                allow_file,
            } => {
                ui.horizontal(|ui| {
                    if *allow_file && ui.button("Select file...").clicked() {
                        if let Some(file) = FileDialog::new().show_open_single_file().ok().flatten()
                        {
                            *value = file.to_string_lossy().into_owned();
                        }
                    }

                    if *allow_dir && ui.button("Select directory...").clicked() {
                        if let Some(file) = FileDialog::new().show_open_single_dir().ok().flatten()
                        {
                            *value = file.to_string_lossy().into_owned();
                        }
                    }

                    if let Some(default) = default {
                        ui.add_space(20.0);
                        if ui.button("Reset to default").clicked() {
                            *value = default.clone();
                        }
                    }

                    ui.error_style_if(validation_error.is(name).is_some(), |ui| {
                        let text = ui.text_edit_singleline(value);

                        if let Some(message) = validation_error.is(name) {
                            if text.on_hover_text(message).changed() {
                                *validation_error = None;
                            }
                        }
                    });
                });
            }
            ArgKind::MultiplePaths {
                values,
                default,
                allow_dir,
                allow_file,
            } => ui.multiple_values(
                validation_error,
                name,
                values,
                Some(default),
                |ui, value| {
                    if *allow_file && ui.button("Select file...").clicked() {
                        if let Some(file) = FileDialog::new().show_open_single_file().ok().flatten()
                        {
                            *value = file.to_string_lossy().into_owned();
                        }
                    };

                    if *allow_dir && ui.button("Select directory...").clicked() {
                        if let Some(file) = FileDialog::new().show_open_single_dir().ok().flatten()
                        {
                            *value = file.to_string_lossy().into_owned();
                        }
                    };

                    ui.text_edit_singleline(value)
                },
            ),
            ArgKind::Choose {
                value: ChooseState(value, id),
                possible,
            } => {
                ComboBox::from_id_source(id)
                    .selected_text(value.clone())
                    .show_ui(ui, |ui| {
                        if *optional {
                            ui.selectable_value(value, String::new(), "None");
                        }
                        for p in possible {
                            ui.selectable_value(value, p.clone(), p);
                        }
                    });
            }
            ArgKind::MultipleChoose {
                values,
                ref possible,
            } => ui.multiple_values(
                validation_error,
                name,
                values,
                None,
                |ui, ChooseState(value, id)| {
                    ComboBox::from_id_source(id)
                        .selected_text(value.clone())
                        .show_ui(ui, |ui| {
                            for p in possible {
                                ui.selectable_value(value, p.clone(), p);
                            }
                        })
                        .response
                },
            ),
        };
    }

    pub fn get_cmd_args(&self, mut args: Vec<String>) -> Result<Vec<String>, String> {
        match &self.kind {
            ArgKind::String { value, .. } => {
                if !value.is_empty() {
                    if let Some(call_name) = self.call_name.as_ref() {
                        if self.use_equals {
                            args.push(format!("{}={}", call_name, value));
                        } else {
                            args.extend_from_slice(&[call_name.clone(), value.clone()]);
                        }
                    } else {
                        args.push(value.clone());
                    }
                } else if !self.optional {
                    return Err(format!("{} is required.", self.name));
                }
            }
            &ArgKind::Occurences(i) => {
                for _ in 0..i {
                    args.push(
                        self.call_name
                            .clone()
                            .ok_or_else(|| "Internal error.".to_string())?,
                    );
                }
            }
            &ArgKind::Bool(bool) => {
                if bool {
                    args.push(
                        self.call_name
                            .clone()
                            .ok_or_else(|| "Internal error.".to_string())?,
                    );
                }
            }
            ArgKind::MultipleStrings { values, .. } => {
                for value in values {
                    if let Some(call_name) = self.call_name.as_ref() {
                        if self.use_equals {
                            args.push(format!("{}={}", call_name, value));
                        } else {
                            args.extend_from_slice(&[call_name.clone(), value.clone()]);
                        }
                    } else {
                        args.push(value.clone());
                    }
                }
            }
            ArgKind::Path { value, .. } => {
                if !value.is_empty() {
                    if let Some(call_name) = self.call_name.as_ref() {
                        if self.use_equals {
                            args.push(format!("{}={}", call_name, value));
                        } else {
                            args.extend_from_slice(&[call_name.clone(), value.clone()]);
                        }
                    } else {
                        args.push(value.clone());
                    }
                } else if !self.optional {
                    return Err(format!("{} is required.", self.name));
                }
            }
            ArgKind::MultiplePaths { values, .. } => {
                for value in values {
                    if let Some(call_name) = self.call_name.as_ref() {
                        if self.use_equals {
                            args.push(format!("{}={}", call_name, value));
                        } else {
                            args.extend_from_slice(&[call_name.clone(), value.clone()]);
                        }
                    } else {
                        args.push(value.clone());
                    }
                }
            }
            ArgKind::Choose {
                value: ChooseState(value, _),
                ..
            } => {
                if !value.is_empty() {
                    if let Some(call_name) = self.call_name.as_ref() {
                        if self.use_equals {
                            args.push(format!("{}={}", call_name, value));
                        } else {
                            args.extend_from_slice(&[call_name.clone(), value.clone()]);
                        }
                    } else {
                        args.push(value.clone());
                    }
                } else if !self.optional {
                    return Err(format!("{} is required.", self.name));
                }
            }
            ArgKind::MultipleChoose { values, .. } => {
                for ChooseState(value, _) in values {
                    if let Some(call_name) = self.call_name.as_ref() {
                        if self.use_equals {
                            args.push(format!("{}={}", call_name, value));
                        } else {
                            args.extend_from_slice(&[call_name.clone(), value.clone()]);
                        }
                    } else {
                        args.push(value.clone());
                    }
                }
            }
        }

        Ok(args)
    }
}

impl From<&Arg<'_>> for ArgState {
    fn from(a: &Arg) -> Self {
        let call_name = a
            .get_long()
            .map(|s| format!("--{}", s))
            .or_else(|| a.get_short().map(|c| format!("-{}", c)));

        let desc = a
            .get_long_about()
            .map(ToString::to_string)
            .or_else(|| a.get_about().map(ToString::to_string));

        let optional =
            !a.is_set(ArgSettings::Required) && !a.is_set(ArgSettings::ForbidEmptyValues);

        use ValueHint::*;
        let kind = match (
            a.is_set(ArgSettings::MultipleOccurrences),
            a.is_set(ArgSettings::TakesValue),
            a.get_value_hint(),
            a.get_possible_values(),
        ) {
            (true, true, AnyPath | DirPath | FilePath | ExecutablePath, None) => {
                let default: Vec<_> = a
                    .get_default_values()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect();

                ArgKind::MultiplePaths {
                    values: default.clone(),
                    default,
                    allow_dir: matches!(a.get_value_hint(), AnyPath | DirPath),
                    allow_file: matches!(a.get_value_hint(), AnyPath | FilePath | ExecutablePath),
                }
            }
            (true, true, _, None) => {
                let default: Vec<_> = a
                    .get_default_values()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect();

                ArgKind::MultipleStrings {
                    values: default.clone(),
                    default,
                }
            }
            (false, true, AnyPath | DirPath | FilePath | ExecutablePath, None) => {
                let default = a
                    .get_default_values()
                    .first()
                    .map(|s| s.to_string_lossy().into_owned());

                ArgKind::Path {
                    value: default.clone().unwrap_or_default(),
                    default,
                    allow_dir: matches!(a.get_value_hint(), AnyPath | DirPath),
                    allow_file: matches!(a.get_value_hint(), AnyPath | FilePath | ExecutablePath),
                }
            }
            (false, true, _, None) => ArgKind::String {
                value: "".into(),
                default: a
                    .get_default_values()
                    .first()
                    .map(|s| s.to_string_lossy().into_owned()),
            },
            (true, false, _, None) => ArgKind::Occurences(0),
            (false, false, _, None) => ArgKind::Bool(false),
            (false, _, _, Some(possible)) => ArgKind::Choose {
                value: ChooseState(
                    if optional {
                        "".into()
                    } else {
                        possible[0].to_string()
                    },
                    Uuid::new_v4(),
                ),
                possible: possible.iter().map(|s| s.to_string()).collect(),
            },
            (true, _, _, Some(possible)) => ArgKind::MultipleChoose {
                values: vec![],
                possible: possible.iter().map(|s| s.to_string()).collect(),
            },
        };

        Self {
            name: a.get_name().to_string().to_sentence_case(),
            call_name,
            desc,
            optional,
            use_equals: a.is_set(ArgSettings::RequireEquals),
            kind,
        }
    }
}
