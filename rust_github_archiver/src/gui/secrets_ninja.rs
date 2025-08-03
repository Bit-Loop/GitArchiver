use iced::{
    widget::{button, column, container, row, text, text_input, scrollable, pick_list, progress_bar},
    Application, Command, Element, Length, Settings, Theme,
    alignment::{Horizontal, Vertical},
    Color, Subscription,
};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::secrets::{SecretMatch, SecretSeverity, SecretCategory, ValidationResult};

#[derive(Debug, Clone)]
pub enum Message {
    LoadSecrets(Vec<SecretMatch>),
    LoadValidationResults(Vec<ValidationResult>),
    FilterBySeverity(SecretSeverity),
    FilterByCategory(SecretCategory),
    SearchTextChanged(String),
    SortBy(SortField),
    ToggleDetails(String), // secret hash
    ValidateSecret(String),
    ExportResults,
    ShowChart(ChartType),
    RefreshData,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortField {
    Severity,
    Category,
    Filename,
    DetectorName,
    Timestamp,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChartType {
    SeverityDistribution,
    CategoryDistribution,
    TimelineLeak,
    FileTypeDistribution,
}

#[derive(Debug, Clone)]
pub struct SecretsNinjaApp {
    secrets: Vec<SecretMatch>,
    validation_results: HashMap<String, ValidationResult>,
    filtered_secrets: Vec<SecretMatch>,
    
    // Filters
    severity_filter: Option<SecretSeverity>,
    category_filter: Option<SecretCategory>,
    search_text: String,
    sort_field: SortField,
    
    // UI State
    expanded_details: std::collections::HashSet<String>,
    current_chart: Option<ChartType>,
    
    // Statistics
    stats: SecretsStatistics,
}

#[derive(Debug, Clone, Default)]
pub struct SecretsStatistics {
    pub total_secrets: usize,
    pub verified_secrets: usize,
    pub severity_counts: HashMap<SecretSeverity, usize>,
    pub category_counts: HashMap<SecretCategory, usize>,
    pub file_type_counts: HashMap<String, usize>,
    pub timeline_data: Vec<(DateTime<Utc>, usize)>,
}

impl Application for SecretsNinjaApp {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                secrets: Vec::new(),
                validation_results: HashMap::new(),
                filtered_secrets: Vec::new(),
                severity_filter: None,
                category_filter: None,
                search_text: String::new(),
                sort_field: SortField::Severity,
                expanded_details: std::collections::HashSet::new(),
                current_chart: None,
                stats: SecretsStatistics::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Secrets Ninja - GitHub Secret Scanner".to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::LoadSecrets(secrets) => {
                self.secrets = secrets;
                self.update_statistics();
                self.apply_filters();
                Command::none()
            }
            Message::LoadValidationResults(results) => {
                for result in results {
                    self.validation_results.insert(result.secret_hash.clone(), result);
                }
                self.update_statistics();
                Command::none()
            }
            Message::FilterBySeverity(severity) => {
                self.severity_filter = Some(severity);
                self.apply_filters();
                Command::none()
            }
            Message::FilterByCategory(category) => {
                self.category_filter = Some(category);
                self.apply_filters();
                Command::none()
            }
            Message::SearchTextChanged(text) => {
                self.search_text = text;
                self.apply_filters();
                Command::none()
            }
            Message::SortBy(field) => {
                self.sort_field = field;
                self.apply_sorting();
                Command::none()
            }
            Message::ToggleDetails(hash) => {
                if self.expanded_details.contains(&hash) {
                    self.expanded_details.remove(&hash);
                } else {
                    self.expanded_details.insert(hash);
                }
                Command::none()
            }
            Message::ValidateSecret(_hash) => {
                // This would trigger validation in the background
                Command::none()
            }
            Message::ExportResults => {
                // This would export the current filtered results
                Command::none()
            }
            Message::ShowChart(chart_type) => {
                self.current_chart = Some(chart_type);
                Command::none()
            }
            Message::RefreshData => {
                // This would reload data from the database
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let header = self.create_header();
        let filters = self.create_filters();
        let statistics = self.create_statistics_panel();
        let secrets_list = self.create_secrets_list();
        
        let content = column![
            header,
            filters,
            row![
                statistics,
                secrets_list
            ].spacing(10)
        ]
        .spacing(10)
        .padding(10);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

impl SecretsNinjaApp {
    fn create_header(&self) -> Element<Message> {
        let title = text("🥷 Secrets Ninja")
            .size(24)
            .color(Color::from_rgb(0.9, 0.1, 0.1));

        let subtitle = text("GitHub Secret Scanner & Validator")
            .size(14)
            .color(Color::from_rgb(0.7, 0.7, 0.7));

        let refresh_button = button("🔄 Refresh")
            .on_press(Message::RefreshData);

        let export_button = button("📁 Export")
            .on_press(Message::ExportResults);

        row![
            column![title, subtitle],
            row![refresh_button, export_button].spacing(10)
        ]
        .align_items(iced::Alignment::Center)
        .into()
    }

    fn create_filters(&self) -> Element<Message> {
        let severity_options = vec![
            SecretSeverity::Low,
            SecretSeverity::Medium,
            SecretSeverity::High,
            SecretSeverity::Critical,
        ];

        let category_options = vec![
            SecretCategory::CloudProvider,
            SecretCategory::Database,
            SecretCategory::ApiKey,
            SecretCategory::Certificate,
            SecretCategory::Password,
            SecretCategory::Token,
            SecretCategory::Webhook,
            SecretCategory::Other,
        ];

        let severity_filter = pick_list(
            severity_options,
            self.severity_filter.clone(),
            Message::FilterBySeverity,
        );

        let category_filter = pick_list(
            category_options,
            self.category_filter.clone(),
            Message::FilterByCategory,
        );

        let search_input = text_input("Search secrets...", &self.search_text)
            .on_input(Message::SearchTextChanged);

        row![
            text("Filters:"),
            severity_filter,
            category_filter,
            search_input
        ]
        .spacing(10)
        .align_items(iced::Alignment::Center)
        .into()
    }

    fn create_statistics_panel(&self) -> Element<Message> {
        let total_text = text(format!("Total Secrets: {}", self.stats.total_secrets));
        let verified_text = text(format!("Verified: {}", self.stats.verified_secrets));
        
        let severity_chart_button = button("📊 Severity Chart")
            .on_press(Message::ShowChart(ChartType::SeverityDistribution));
        
        let category_chart_button = button("📈 Category Chart")
            .on_press(Message::ShowChart(ChartType::CategoryDistribution));
        
        let timeline_chart_button = button("📉 Timeline")
            .on_press(Message::ShowChart(ChartType::TimelineLeak));

        let severity_breakdown = self.create_severity_breakdown();
        let category_breakdown = self.create_category_breakdown();

        column![
            text("Statistics").size(18),
            total_text,
            verified_text,
            severity_breakdown,
            category_breakdown,
            column![
                severity_chart_button,
                category_chart_button,
                timeline_chart_button
            ].spacing(5)
        ]
        .spacing(10)
        .width(Length::Fixed(300.0))
        .into()
    }

    fn create_severity_breakdown(&self) -> Element<Message> {
        let mut breakdown = column!["Severity Breakdown:"];
        
        for (severity, count) in &self.stats.severity_counts {
            let percentage = if self.stats.total_secrets > 0 {
                (*count as f32 / self.stats.total_secrets as f32) * 100.0
            } else {
                0.0
            };
            
            let color = match severity {
                SecretSeverity::Critical => Color::from_rgb(0.9, 0.1, 0.1),
                SecretSeverity::High => Color::from_rgb(0.9, 0.5, 0.1),
                SecretSeverity::Medium => Color::from_rgb(0.9, 0.9, 0.1),
                SecretSeverity::Low => Color::from_rgb(0.1, 0.9, 0.1),
            };
            
            let severity_text = text(format!("{:?}: {} ({:.1}%)", severity, count, percentage))
                .color(color);
            
            let progress = progress_bar(0.0..=100.0, percentage);
            
            breakdown = breakdown.push(column![severity_text, progress].spacing(2));
        }
        
        breakdown.spacing(5).into()
    }

    fn create_category_breakdown(&self) -> Element<Message> {
        let mut breakdown = column!["Category Breakdown:"];
        
        for (category, count) in &self.stats.category_counts {
            let percentage = if self.stats.total_secrets > 0 {
                (*count as f32 / self.stats.total_secrets as f32) * 100.0
            } else {
                0.0
            };
            
            let category_text = text(format!("{:?}: {} ({:.1}%)", category, count, percentage));
            let progress = progress_bar(0.0..=100.0, percentage);
            
            breakdown = breakdown.push(column![category_text, progress].spacing(2));
        }
        
        breakdown.spacing(5).into()
    }

    fn create_secrets_list(&self) -> Element<Message> {
        let mut list = column![];
        
        for secret in &self.filtered_secrets {
            let secret_item = self.create_secret_item(secret);
            list = list.push(secret_item);
        }
        
        scrollable(list)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn create_secret_item(&self, secret: &SecretMatch) -> Element<Message> {
        let severity_color = match secret.severity {
            SecretSeverity::Critical => Color::from_rgb(0.9, 0.1, 0.1),
            SecretSeverity::High => Color::from_rgb(0.9, 0.5, 0.1),
            SecretSeverity::Medium => Color::from_rgb(0.9, 0.9, 0.1),
            SecretSeverity::Low => Color::from_rgb(0.1, 0.9, 0.1),
        };

        let detector_name = text(&secret.detector_name)
            .size(16)
            .color(severity_color);

        let filename = text(secret.filename.as_deref().unwrap_or("unknown"))
            .size(12)
            .color(Color::from_rgb(0.7, 0.7, 0.7));

        let severity_badge = text(format!("{:?}", secret.severity))
            .size(10)
            .color(severity_color);

        let category_badge = text(format!("{:?}", secret.category))
            .size(10)
            .color(Color::from_rgb(0.5, 0.5, 0.9));

        let validation_status = if let Some(validation) = self.validation_results.get(&secret.hash) {
            if validation.is_valid {
                text("✅ Verified")
                    .size(10)
                    .color(Color::from_rgb(0.1, 0.9, 0.1))
            } else {
                text("❌ Invalid")
                    .size(10)
                    .color(Color::from_rgb(0.9, 0.1, 0.1))
            }
        } else {
            text("🔍 Validate")
                .size(10)
                .color(Color::from_rgb(0.5, 0.5, 0.5))
        };

        let validate_button = button(validation_status)
            .on_press(Message::ValidateSecret(secret.hash.clone()));

        let details_button = button(if self.expanded_details.contains(&secret.hash) {
            "🔽 Hide Details"
        } else {
            "🔼 Show Details"
        })
        .on_press(Message::ToggleDetails(secret.hash.clone()));

        let main_row = row![
            column![detector_name, filename],
            row![severity_badge, category_badge].spacing(5),
            validate_button,
            details_button
        ]
        .spacing(10)
        .align_items(iced::Alignment::Center);

        let mut item = column![main_row];

        if self.expanded_details.contains(&secret.hash) {
            let details = self.create_secret_details(secret);
            item = item.push(details);
        }

        container(item)
            .padding(10)
            .style(iced::theme::Container::Box)
            .width(Length::Fill)
            .into()
    }

    fn create_secret_details(&self, secret: &SecretMatch) -> Element<Message> {
        let matched_text = text(&secret.matched_text)
            .size(12)
            .color(Color::from_rgb(0.9, 0.9, 0.9));

        let entropy_text = text(format!("Entropy: {:.2}", secret.entropy))
            .size(10)
            .color(Color::from_rgb(0.7, 0.7, 0.7));

        let line_text = if let Some(line) = secret.line_number {
            text(format!("Line: {}", line))
                .size(10)
                .color(Color::from_rgb(0.7, 0.7, 0.7))
        } else {
            text("")
        };

        let context_text = text(&secret.context)
            .size(10)
            .color(Color::from_rgb(0.6, 0.6, 0.6));

        let validation_details = if let Some(validation) = self.validation_results.get(&secret.hash) {
            column![
                text(format!("Validation Method: {}", validation.validation_method))
                    .size(10),
                text(format!("Validated At: {}", validation.validated_at.format("%Y-%m-%d %H:%M:%S")))
                    .size(10),
                if let Some(info) = &validation.additional_info {
                    text(format!("Info: {}", info)).size(10)
                } else {
                    text("")
                },
                if let Some(error) = &validation.error_message {
                    text(format!("Error: {}", error))
                        .size(10)
                        .color(Color::from_rgb(0.9, 0.1, 0.1))
                } else {
                    text("")
                }
            ]
            .spacing(2)
        } else {
            column![]
        };

        column![
            text("Matched Text:").size(12),
            matched_text,
            row![entropy_text, line_text].spacing(10),
            text("Context:").size(12),
            context_text,
            validation_details
        ]
        .spacing(5)
        .padding(10)
        .into()
    }

    fn apply_filters(&mut self) {
        self.filtered_secrets = self.secrets
            .iter()
            .filter(|secret| {
                // Severity filter
                if let Some(severity_filter) = &self.severity_filter {
                    if &secret.severity != severity_filter {
                        return false;
                    }
                }

                // Category filter
                if let Some(category_filter) = &self.category_filter {
                    if &secret.category != category_filter {
                        return false;
                    }
                }

                // Search text filter
                if !self.search_text.is_empty() {
                    let search_lower = self.search_text.to_lowercase();
                    let matches_text = secret.matched_text.to_lowercase().contains(&search_lower)
                        || secret.detector_name.to_lowercase().contains(&search_lower)
                        || secret.filename.as_ref().map_or(false, |f| f.to_lowercase().contains(&search_lower));
                    
                    if !matches_text {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        self.apply_sorting();
    }

    fn apply_sorting(&mut self) {
        self.filtered_secrets.sort_by(|a, b| {
            match self.sort_field {
                SortField::Severity => {
                    let a_level = match a.severity {
                        SecretSeverity::Critical => 3,
                        SecretSeverity::High => 2,
                        SecretSeverity::Medium => 1,
                        SecretSeverity::Low => 0,
                    };
                    let b_level = match b.severity {
                        SecretSeverity::Critical => 3,
                        SecretSeverity::High => 2,
                        SecretSeverity::Medium => 1,
                        SecretSeverity::Low => 0,
                    };
                    b_level.cmp(&a_level) // Descending (highest severity first)
                }
                SortField::Category => a.category.to_string().cmp(&b.category.to_string()),
                SortField::Filename => {
                    let a_file = a.filename.as_deref().unwrap_or("");
                    let b_file = b.filename.as_deref().unwrap_or("");
                    a_file.cmp(b_file)
                }
                SortField::DetectorName => a.detector_name.cmp(&b.detector_name),
                SortField::Timestamp => {
                    // For timestamp sorting, we'd need to add timestamp field to SecretMatch
                    a.detector_name.cmp(&b.detector_name)
                }
            }
        });
    }

    fn update_statistics(&mut self) {
        self.stats.total_secrets = self.secrets.len();
        self.stats.verified_secrets = self.validation_results.len();
        
        // Count by severity
        self.stats.severity_counts.clear();
        for secret in &self.secrets {
            *self.stats.severity_counts.entry(secret.severity.clone()).or_insert(0) += 1;
        }
        
        // Count by category
        self.stats.category_counts.clear();
        for secret in &self.secrets {
            *self.stats.category_counts.entry(secret.category.clone()).or_insert(0) += 1;
        }
        
        // Count by file type
        self.stats.file_type_counts.clear();
        for secret in &self.secrets {
            if let Some(filename) = &secret.filename {
                let extension = std::path::Path::new(filename)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                *self.stats.file_type_counts.entry(extension).or_insert(0) += 1;
            }
        }
    }
}

// Implement Display for enum types to make them work with pick_list
impl std::fmt::Display for SecretSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::fmt::Display for SecretCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Launch the Secrets Ninja GUI
pub fn launch_secrets_ninja() -> iced::Result {
    SecretsNinjaApp::run(Settings::default())
}

/// Load secrets data into the GUI
pub fn load_secrets_data(secrets: Vec<SecretMatch>, validations: Vec<ValidationResult>) -> SecretsNinjaApp {
    let mut app = SecretsNinjaApp {
        secrets: secrets.clone(),
        validation_results: HashMap::new(),
        filtered_secrets: Vec::new(),
        severity_filter: None,
        category_filter: None,
        search_text: String::new(),
        sort_field: SortField::Severity,
        expanded_details: std::collections::HashSet::new(),
        current_chart: None,
        stats: SecretsStatistics::default(),
    };

    // Load validation results
    for validation in validations {
        app.validation_results.insert(validation.secret_hash.clone(), validation);
    }

    app.update_statistics();
    app.apply_filters();
    
    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{SecretMatch, SecretSeverity, SecretCategory};

    fn create_test_secret() -> SecretMatch {
        SecretMatch {
            detector_name: "Test Detector".to_string(),
            matched_text: "test_secret_123".to_string(),
            start_position: 0,
            end_position: 15,
            line_number: Some(42),
            filename: Some("test.env".to_string()),
            entropy: 4.5,
            severity: SecretSeverity::High,
            category: SecretCategory::ApiKey,
            context: "api_key = 'test_secret_123'".to_string(),
            verified: false,
            hash: "test_hash_123".to_string(),
        }
    }

    #[test]
    fn test_app_creation() {
        let (app, _) = SecretsNinjaApp::new(());
        assert_eq!(app.secrets.len(), 0);
        assert_eq!(app.stats.total_secrets, 0);
    }

    #[test]
    fn test_statistics_update() {
        let secrets = vec![
            create_test_secret(),
            SecretMatch {
                severity: SecretSeverity::Critical,
                category: SecretCategory::Database,
                ..create_test_secret()
            },
        ];

        let app = load_secrets_data(secrets, vec![]);
        assert_eq!(app.stats.total_secrets, 2);
        assert_eq!(app.stats.severity_counts.get(&SecretSeverity::High), Some(&1));
        assert_eq!(app.stats.severity_counts.get(&SecretSeverity::Critical), Some(&1));
        assert_eq!(app.stats.category_counts.get(&SecretCategory::ApiKey), Some(&1));
        assert_eq!(app.stats.category_counts.get(&SecretCategory::Database), Some(&1));
    }

    #[test]
    fn test_filtering() {
        let mut app = load_secrets_data(vec![create_test_secret()], vec![]);
        
        // Test severity filter
        app.severity_filter = Some(SecretSeverity::High);
        app.apply_filters();
        assert_eq!(app.filtered_secrets.len(), 1);
        
        app.severity_filter = Some(SecretSeverity::Low);
        app.apply_filters();
        assert_eq!(app.filtered_secrets.len(), 0);
        
        // Test search filter
        app.severity_filter = None;
        app.search_text = "test".to_string();
        app.apply_filters();
        assert_eq!(app.filtered_secrets.len(), 1);
        
        app.search_text = "nonexistent".to_string();
        app.apply_filters();
        assert_eq!(app.filtered_secrets.len(), 0);
    }
}
